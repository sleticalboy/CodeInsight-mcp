use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser, TreeCursor};

use crate::{
    config::{IndexConfig, ProjectConfig, load_project_config},
    language::{detect_language, tree_sitter_language},
    model::{
        CallEdge, Dependency, IndexError, IndexScopeReport, Language, ProjectIndexReport,
        SourceFile, Symbol, SymbolKind,
    },
    storage::{INDEX_VERSION, SCHEMA_VERSION, Store},
};

const DEFAULT_PACKAGE_CONDITIONS: &[&str] = &[
    "import",
    "node",
    "browser",
    "default",
    "require",
    "development",
    "production",
];

fn default_package_conditions() -> Vec<String> {
    DEFAULT_PACKAGE_CONDITIONS
        .iter()
        .map(|condition| condition.to_string())
        .collect()
}

fn project_package_conditions(config: &ProjectConfig) -> Vec<String> {
    if config.javascript.package_conditions.is_empty() {
        default_package_conditions()
    } else {
        config.javascript.package_conditions.clone()
    }
}

pub fn index_project(root: &Path, force: bool) -> Result<ProjectIndexReport> {
    let started = Instant::now();
    let root = root.canonicalize()?;
    let project_config = load_project_config(&root)?.unwrap_or_default();
    let package_conditions = project_package_conditions(&project_config);
    let path_scope = IndexPathScope::from_config(&project_config.index)?;
    let mut store = Store::open(&root)?;
    if force {
        store.reset()?;
    }

    let mut changed_files = 0;
    let mut unchanged_files = 0;
    let mut skipped_files = 0;
    let mut symbol_count = 0;
    let mut seen_source_files = Vec::new();
    let mut errors = Vec::new();

    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .filter_entry(|entry| should_enter(entry.path()))
        .build()
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(IndexError {
                    file: "<walk>".to_string(),
                    stage: "walk".to_string(),
                    message: error.to_string(),
                });
                skipped_files += 1;
                continue;
            }
        };
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }

        let path = entry.path();
        let relative_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if !path_scope.matches(&relative_path) {
            skipped_files += 1;
            continue;
        }

        let Some(language) = detect_language(path) else {
            skipped_files += 1;
            continue;
        };

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                errors.push(index_error(path, "read", error));
                skipped_files += 1;
                continue;
            }
        };

        seen_source_files.push(relative_path.clone());
        let hash = hash_source(&source);
        if !force && store.file_hash(&relative_path)?.as_deref() == Some(hash.as_str()) {
            unchanged_files += 1;
            continue;
        }

        let symbols = match extract_symbols(&source, language, &relative_path) {
            Ok(symbols) => symbols,
            Err(error) => {
                errors.push(IndexError {
                    file: relative_path.clone(),
                    stage: "parse_symbols".to_string(),
                    message: error.to_string(),
                });
                skipped_files += 1;
                continue;
            }
        };
        let mut dependencies = match extract_dependencies(&source, language, &relative_path) {
            Ok(dependencies) => dependencies,
            Err(error) => {
                errors.push(IndexError {
                    file: relative_path.clone(),
                    stage: "parse_dependencies".to_string(),
                    message: error.to_string(),
                });
                skipped_files += 1;
                continue;
            }
        };
        resolve_dependencies(&root, &mut dependencies, &package_conditions);
        let calls = extract_calls(&source, language, &relative_path, &symbols);
        let source_file = SourceFile {
            path: path.to_path_buf(),
            relative_path: relative_path.clone(),
            language,
            hash,
            line_count: source.lines().count(),
        };
        let file_id = match store.upsert_file(&source_file) {
            Ok(file_id) => file_id,
            Err(error) => {
                errors.push(IndexError {
                    file: relative_path.clone(),
                    stage: "store_file".to_string(),
                    message: error.to_string(),
                });
                skipped_files += 1;
                continue;
            }
        };
        if let Err(error) = store.replace_symbols(file_id, &symbols) {
            errors.push(IndexError {
                file: relative_path.clone(),
                stage: "store_symbols".to_string(),
                message: error.to_string(),
            });
            skipped_files += 1;
            continue;
        }
        if let Err(error) = store.replace_dependencies(file_id, &dependencies) {
            errors.push(IndexError {
                file: relative_path.clone(),
                stage: "store_dependencies".to_string(),
                message: error.to_string(),
            });
            skipped_files += 1;
            continue;
        }
        if let Err(error) = store.replace_calls(file_id, &calls) {
            errors.push(IndexError {
                file: relative_path.clone(),
                stage: "store_calls".to_string(),
                message: error.to_string(),
            });
            skipped_files += 1;
            continue;
        }

        changed_files += 1;
        symbol_count += symbols.len();
    }
    let deleted_files = store.delete_files_not_in(&seen_source_files)?;
    store.resolve_imported_calls()?;
    let total_indexed_files = store.count_files()?;
    let total_symbols = store.count_symbols()?;
    store.mark_indexed()?;

    Ok(ProjectIndexReport {
        root: root.display().to_string(),
        schema_version: SCHEMA_VERSION,
        index_version: INDEX_VERSION.to_string(),
        index_scope: index_scope_report(&project_config.index),
        indexed_files: total_indexed_files,
        changed_files,
        unchanged_files,
        deleted_files,
        skipped_files,
        symbols: total_symbols,
        changed_symbols: symbol_count,
        errors,
        duration_ms: started.elapsed().as_millis(),
    })
}

fn index_scope_report(config: &IndexConfig) -> IndexScopeReport {
    IndexScopeReport {
        enabled: !config.include.is_empty() || !config.exclude.is_empty(),
        includes: config.include.clone(),
        excludes: config.exclude.clone(),
    }
}

#[derive(Debug, Default)]
struct IndexPathScope {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl IndexPathScope {
    fn from_config(config: &IndexConfig) -> Result<Self> {
        Ok(Self {
            include: build_scope_glob_set(&config.include, "index.include")?,
            exclude: build_scope_glob_set(&config.exclude, "index.exclude")?,
        })
    }

    fn matches(&self, relative_path: &str) -> bool {
        let included = match &self.include {
            Some(include) => include.is_match(relative_path),
            None => true,
        };
        let excluded = self
            .exclude
            .as_ref()
            .is_some_and(|exclude| exclude.is_match(relative_path));
        included && !excluded
    }
}

fn build_scope_glob_set(patterns: &[String], label: &str) -> Result<Option<GlobSet>> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;

    for raw_pattern in patterns {
        let pattern = normalize_scope_pattern(raw_pattern);
        if pattern.is_empty() {
            continue;
        }

        add_scope_glob(&mut builder, &pattern, label)?;
        if !contains_glob_meta(&pattern) {
            add_scope_glob(&mut builder, &format!("{pattern}/**"), label)?;
        }
        added = true;
    }

    if added {
        Ok(Some(builder.build()?))
    } else {
        Ok(None)
    }
}

fn add_scope_glob(builder: &mut GlobSetBuilder, pattern: &str, label: &str) -> Result<()> {
    let glob =
        Glob::new(pattern).with_context(|| format!("invalid {label} glob pattern `{pattern}`"))?;
    builder.add(glob);
    Ok(())
}

fn normalize_scope_pattern(pattern: &str) -> String {
    let mut pattern = pattern.trim().replace('\\', "/");
    while let Some(stripped) = pattern.strip_prefix("./") {
        pattern = stripped.to_string();
    }
    while let Some(stripped) = pattern.strip_prefix('/') {
        pattern = stripped.to_string();
    }
    pattern.trim_end_matches('/').to_string()
}

fn contains_glob_meta(pattern: &str) -> bool {
    pattern
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'{'))
}

fn index_error(path: &Path, stage: &str, error: impl std::fmt::Display) -> IndexError {
    IndexError {
        file: path.display().to_string(),
        stage: stage.to_string(),
        message: error.to_string(),
    }
}

pub fn outline_file(path: &Path) -> Result<Vec<Symbol>> {
    let language = detect_language(path).context("unsupported file language")?;
    let source = fs::read_to_string(path)?;
    let file = path.to_string_lossy().to_string();
    extract_symbols(&source, language, &file)
}

pub fn extract_symbols(source: &str, language: Language, file: &str) -> Result<Vec<Symbol>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_language(language))?;
    let tree = parser
        .parse(source, None)
        .context("tree-sitter parse failed")?;
    let mut symbols = Vec::new();
    let mut scope = Vec::new();
    visit_node(
        tree.root_node(),
        source.as_bytes(),
        language,
        file,
        &mut scope,
        &mut symbols,
    );
    Ok(symbols)
}

pub fn extract_dependencies(
    source: &str,
    language: Language,
    source_file: &str,
) -> Result<Vec<Dependency>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_language(language))?;
    let tree = parser
        .parse(source, None)
        .context("tree-sitter parse failed")?;
    let javascript_bindings = if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        Some(javascript_static_string_bindings(
            tree.root_node(),
            source.as_bytes(),
        ))
    } else {
        None
    };
    let mut dependencies = Vec::new();
    visit_dependency_node(
        tree.root_node(),
        source.as_bytes(),
        language,
        source_file,
        javascript_bindings.as_ref(),
        &mut dependencies,
    );
    Ok(dependencies)
}

pub fn extract_calls(
    source: &str,
    language: Language,
    source_file: &str,
    symbols: &[Symbol],
) -> Vec<CallEdge> {
    let mut parser = Parser::new();
    if parser
        .set_language(&tree_sitter_language(language))
        .is_err()
    {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut calls = Vec::new();
    visit_call_node(
        tree.root_node(),
        source.as_bytes(),
        language,
        source_file,
        symbols,
        &mut calls,
    );
    calls
}

fn visit_call_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    symbols: &[Symbol],
    calls: &mut Vec<CallEdge>,
) {
    if is_call_node(node, language)
        && !should_skip_call_node(node, language)
        && let Some(raw_target) = call_target_text(node, source, language)
        && let Some(callee) = normalize_callee(&raw_target, language)
    {
        let line = node.start_position().row + 1;
        let caller = caller_for_call_node(node, source, language, symbols, line)
            .unwrap_or_else(|| "<module>".to_string());
        let callee_file = if matches!(language, Language::JavaScript | Language::TypeScript) {
            None
        } else {
            same_file_callee_file(&callee, source_file, symbols)
        };
        let confidence = if callee_file.is_some() { 0.72 } else { 0.55 };
        calls.push(CallEdge {
            file: source_file.to_string(),
            caller,
            callee,
            callee_file,
            language,
            line,
            column: node.start_position().column + 1,
            confidence,
        });
    }

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        visit_call_node(child, source, language, source_file, symbols, calls);
    }
}

fn same_file_callee_file(callee: &str, source_file: &str, symbols: &[Symbol]) -> Option<String> {
    symbols
        .iter()
        .any(|symbol| {
            symbol.name == callee
                || symbol.qualified_name == callee
                || symbol
                    .qualified_name
                    .rsplit('.')
                    .next()
                    .map(|name| name == callee)
                    .unwrap_or(false)
        })
        .then(|| source_file.to_string())
}

fn should_skip_call_node(node: Node<'_>, language: Language) -> bool {
    language == Language::CSharp
        && node.kind() == "invocation_expression"
        && csharp_call_target_has_nested_invocation(node)
}

fn csharp_call_target_has_nested_invocation(node: Node<'_>) -> bool {
    node.child_by_field_name("function")
        .or_else(|| node.child_by_field_name("name"))
        .or_else(|| node.child(0))
        .is_some_and(|target| has_descendant_kind(target, "invocation_expression"))
}

fn has_descendant_kind(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == kind || has_descendant_kind(child, kind) {
            return true;
        }
    }
    false
}

fn is_call_node(node: Node<'_>, language: Language) -> bool {
    match language {
        Language::Bash => node.kind() == "command",
        Language::C
        | Language::Cpp
        | Language::Go
        | Language::Rust
        | Language::JavaScript
        | Language::TypeScript
        | Language::Tsx => node.kind() == "call_expression",
        Language::CSharp => matches!(
            node.kind(),
            "invocation_expression" | "object_creation_expression"
        ),
        Language::Java => matches!(
            node.kind(),
            "method_invocation" | "object_creation_expression"
        ),
        Language::Php => matches!(
            node.kind(),
            "function_call_expression"
                | "member_call_expression"
                | "nullsafe_member_call_expression"
                | "object_creation_expression"
                | "scoped_call_expression"
        ),
        Language::Ruby => node.kind() == "call",
        Language::Python => node.kind() == "call",
    }
}

fn call_target_text(node: Node<'_>, source: &[u8], language: Language) -> Option<String> {
    if language == Language::Bash {
        return child_text(node, "name", source);
    }
    if language == Language::Php
        && let Some(target) = php_call_target_text(node, source)
    {
        return Some(target);
    }
    if language == Language::Ruby
        && let Some(target) = ruby_call_target_text(node, source)
    {
        return Some(target);
    }
    if language == Language::Java
        && let Some(target) = java_call_target_text(node, source)
    {
        return Some(target);
    }
    if language == Language::CSharp
        && let Some(target) = csharp_call_target_text(node, source)
    {
        return Some(target);
    }

    node.child_by_field_name("function")
        .or_else(|| node.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("type"))
        .or_else(|| node.child(0))
        .and_then(|child| child.utf8_text(source).ok())
        .map(ToOwned::to_owned)
}

fn ruby_call_target_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    if let (Some(receiver), Some(method)) = (
        child_text(node, "receiver", source),
        child_text(node, "method", source),
    ) {
        return Some(format!("{receiver}.{method}"));
    }

    child_text(node, "method", source)
        .or_else(|| child_text(node, "name", source))
        .or_else(|| {
            node.child(0)
                .and_then(|child| child.utf8_text(source).ok())
                .map(ToOwned::to_owned)
        })
}

fn php_call_target_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "function_call_expression" => child_text(node, "function", source),
        "member_call_expression" | "nullsafe_member_call_expression" => {
            child_text(node, "name", source)
        }
        "scoped_call_expression" => {
            let scope = child_text(node, "scope", source)?;
            let name = child_text(node, "name", source)?;
            Some(format!("{scope}::{name}"))
        }
        "object_creation_expression" => {
            first_child_text(node, source, &["name", "qualified_name", "relative_name"])
        }
        _ => None,
    }
}

fn java_call_target_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "method_invocation" => {
            let name = child_text(node, "name", source)?;
            if let Some(object) = child_text(node, "object", source) {
                Some(format!("{object}.{name}"))
            } else {
                Some(name)
            }
        }
        "object_creation_expression" => child_text(node, "type", source).or_else(|| {
            node.child(0)
                .and_then(|child| child.utf8_text(source).ok())
                .map(ToOwned::to_owned)
        }),
        _ => None,
    }
}

fn csharp_call_target_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "invocation_expression" => node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.child(0))
            .and_then(|child| child.utf8_text(source).ok())
            .map(ToOwned::to_owned),
        "object_creation_expression" => child_text(node, "type", source).or_else(|| {
            node.child(0)
                .and_then(|child| child.utf8_text(source).ok())
                .map(ToOwned::to_owned)
        }),
        _ => None,
    }
}

fn normalize_callee(raw: &str, language: Language) -> Option<String> {
    if language == Language::Bash {
        return normalize_bash_callee(raw);
    }
    if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        return normalize_javascript_callee(raw);
    }
    if language == Language::Python {
        return normalize_python_callee(raw);
    }
    if language == Language::Rust {
        return normalize_rust_callee(raw);
    }
    if language == Language::Go {
        return normalize_go_callee(raw);
    }
    if language == Language::Java {
        return normalize_java_callee(raw);
    }
    if language == Language::CSharp {
        return normalize_csharp_callee(raw);
    }
    if language == Language::Php {
        return normalize_php_callee(raw);
    }
    if language == Language::Ruby {
        return normalize_ruby_callee(raw);
    }

    normalize_simple_callee(raw)
}

fn normalize_bash_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('$') {
        return None;
    }
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let name =
        name.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`') || ch.is_ascii_whitespace());
    if name.is_empty()
        || name
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
    {
        return None;
    }
    Some(name.to_string())
}

fn normalize_javascript_callee(raw: &str) -> Option<String> {
    let trimmed = raw
        .trim()
        .replace("?.[", "[")
        .replace("?.(", "(")
        .replace("?.", ".");
    if trimmed.is_empty() {
        return None;
    }

    normalize_js_member_path(&trimmed).or_else(|| normalize_simple_callee(&trimmed))
}

fn normalize_js_member_path(raw: &str) -> Option<String> {
    let mut parts = Vec::new();
    for segment in split_js_member_segments(raw) {
        append_js_member_segment(segment, &mut parts)?;
    }

    if parts.is_empty() {
        return None;
    }

    Some(parts.join("."))
}

fn split_js_member_segments(raw: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in raw.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '.' if paren_depth == 0 && bracket_depth == 0 => {
                segments.push(raw[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    segments.push(raw[start..].trim());
    segments
}

fn append_js_member_segment(segment: &str, parts: &mut Vec<String>) -> Option<()> {
    let segment = segment.trim().trim_end_matches('?').trim();
    if segment.is_empty() {
        return None;
    }

    if let Some(open) = top_level_index(segment, '[') {
        let base = segment[..open].trim();
        if !base.is_empty() {
            parts.push(normalize_js_call_or_identifier_segment(base)?);
        }

        let close = segment.rfind(']')?;
        if close <= open {
            return None;
        }

        let property = segment[open + 1..close].trim();
        if let Some(property) = string_literal_value(property)
            && is_js_identifier(&property)
        {
            parts.push(property);
        } else if is_js_identifier(property) {
            parts.push("<dynamic>".to_string());
        } else {
            return None;
        }

        return Some(());
    }

    parts.push(normalize_js_call_or_identifier_segment(segment)?);
    Some(())
}

fn normalize_js_call_or_identifier_segment(segment: &str) -> Option<String> {
    let segment = segment.trim().trim_end_matches('?').trim();
    let name = if let Some(open) = top_level_index(segment, '(') {
        let close = segment.rfind(')')?;
        if close <= open {
            return None;
        }
        segment[..open].trim()
    } else {
        segment
    };

    if is_js_identifier(name) {
        Some(name.to_string())
    } else {
        None
    }
}

fn normalize_python_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = trimmed
        .split('.')
        .map(str::trim)
        .map(|part| part.trim_end_matches('?'))
        .filter(|part| !part.is_empty())
        .map(|part| {
            if let Some(open) = part.find('(') {
                part[..open].trim()
            } else {
                part
            }
        })
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| !is_js_identifier(part)) {
        return normalize_simple_callee(trimmed);
    }

    Some(parts.join("."))
}

fn normalize_rust_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = trimmed
        .split("::")
        .map(str::trim)
        .map(|part| {
            if let Some(open) = part.find('(') {
                part[..open].trim()
            } else {
                part
            }
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| !is_js_identifier(part)) {
        return normalize_simple_callee(trimmed);
    }

    Some(parts.join("."))
}

fn normalize_go_callee(raw: &str) -> Option<String> {
    normalize_dot_callee(raw)
}

fn normalize_java_callee(raw: &str) -> Option<String> {
    normalize_dot_callee(raw)
}

fn normalize_csharp_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim().replace("?.", ".");
    if let Some(callee) = normalize_csharp_new_chain(&trimmed) {
        return Some(callee);
    }
    if trimmed.starts_with("new ") {
        return None;
    }
    normalize_csharp_dot_callee(trimmed.strip_prefix("this.").unwrap_or(&trimmed))
}

fn normalize_csharp_new_chain(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let rest = raw.strip_prefix("new ")?;
    let paren_open = top_level_index(rest, '(');
    let brace_open = top_level_index(rest, '{');
    let type_end = match (paren_open, brace_open) {
        (Some(paren_open), Some(brace_open)) => paren_open.min(brace_open),
        (Some(paren_open), None) => paren_open,
        (None, Some(brace_open)) => brace_open,
        (None, None) => return None,
    };
    let raw_type = rest[..type_end].trim();
    let mut after_new = &rest[type_end..];
    if after_new.trim_start().starts_with('(') {
        let open = after_new.find('(')?;
        let close = matching_close_paren(after_new, open)?;
        after_new = &after_new[close + 1..];
    }
    after_new = after_new.trim_start();
    if after_new.starts_with('{') {
        let close = matching_close_brace(after_new, 0)?;
        after_new = &after_new[close + 1..];
    }
    after_new = strip_leading_csharp_indexers(after_new)?;
    let member_tail = after_new.trim().strip_prefix('.')?.trim();
    let (target, member_tail) =
        if let Some((target, wrapper_member)) = csharp_value_wrapper_type(raw_type) {
            let member_tail = member_tail
                .strip_prefix(wrapper_member)?
                .strip_prefix('.')?;
            (clean_csharp_type_name(target)?, member_tail)
        } else {
            (clean_csharp_type_name(raw_type)?, member_tail)
        };
    let member_tail = normalize_csharp_dot_callee(member_tail)?;
    Some(format!("{target}.{member_tail}"))
}

fn strip_leading_csharp_indexers(mut raw: &str) -> Option<&str> {
    loop {
        raw = raw.trim_start();
        if !raw.starts_with('[') {
            return Some(raw);
        }
        let close = matching_close_bracket(raw, 0)?;
        raw = &raw[close + 1..];
    }
}

fn matching_close_paren(raw: &str, open: usize) -> Option<usize> {
    matching_close_delimiter(raw, open, '(', ')')
}

fn matching_close_brace(raw: &str, open: usize) -> Option<usize> {
    matching_close_delimiter(raw, open, '{', '}')
}

fn matching_close_bracket(raw: &str, open: usize) -> Option<usize> {
    matching_close_delimiter(raw, open, '[', ']')
}

fn matching_close_delimiter(
    raw: &str,
    open: usize,
    open_delimiter: char,
    close_delimiter: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in raw.char_indices().skip_while(|(index, _)| *index < open) {
        if character == open_delimiter {
            depth += 1;
        } else if character == close_delimiter {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn normalize_csharp_dot_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = trimmed
        .split('.')
        .map(str::trim)
        .map(|part| {
            let part = if let Some(open) = part.find('(') {
                part[..open].trim()
            } else {
                part
            };
            csharp_callee_segment_name(part)
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| !is_js_identifier(part)) {
        return normalize_simple_callee(trimmed);
    }

    Some(parts.join("."))
}

fn csharp_callee_segment_name(segment: &str) -> &str {
    let segment = segment.trim().trim_end_matches(['?', '!']).trim();
    let segment = segment
        .split_once('[')
        .map(|(name, _)| name.trim())
        .unwrap_or(segment);
    segment
        .split_once('<')
        .map(|(name, _)| name.trim())
        .unwrap_or(segment)
}

fn normalize_php_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('\\').replace("::", ".");
    normalize_dot_callee(&trimmed)
}

fn normalize_ruby_callee(raw: &str) -> Option<String> {
    normalize_dot_callee(raw)
}

fn normalize_dot_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = trimmed
        .split('.')
        .map(str::trim)
        .map(|part| {
            if let Some(open) = part.find('(') {
                part[..open].trim()
            } else {
                part
            }
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| !is_js_identifier(part)) {
        return normalize_simple_callee(trimmed);
    }

    Some(parts.join("."))
}

fn top_level_index(raw: &str, needle: char) -> Option<usize> {
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in raw.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' if needle != '(' => paren_depth += 1,
            ')' if needle != ')' && paren_depth > 0 => paren_depth -= 1,
            '[' if needle != '[' => bracket_depth += 1,
            ']' if needle != ']' && bracket_depth > 0 => bracket_depth -= 1,
            _ => {}
        }

        if ch == needle && paren_depth == 0 && bracket_depth == 0 {
            return Some(index);
        }
    }

    None
}

fn string_literal_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let mut chars = trimmed.chars();
    let quote = chars.next()?;
    if !matches!(quote, '"' | '\'' | '`') || !trimmed.ends_with(quote) || trimmed.len() < 2 {
        return None;
    }
    if quote == '`' && trimmed.contains("${") {
        return None;
    }

    Some(trimmed[quote.len_utf8()..trimmed.len() - quote.len_utf8()].to_string())
}

fn normalize_simple_callee(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = trimmed
        .rsplit(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .find(|part| !part.is_empty())?;
    if candidate
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn caller_for_line(symbols: &[Symbol], line: usize) -> Option<String> {
    symbols
        .iter()
        .filter(|symbol| {
            matches!(symbol.kind, SymbolKind::Function | SymbolKind::Method)
                && symbol.start_line <= line
                && line <= symbol.end_line
        })
        .max_by_key(|symbol| symbol.start_line)
        .map(|symbol| symbol.qualified_name.clone())
}

fn caller_for_call_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    symbols: &[Symbol],
    line: usize,
) -> Option<String> {
    if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) && let Some(caller) = javascript_callback_caller(node, source, language)
    {
        return Some(caller);
    }

    caller_for_line(symbols, line)
}

fn javascript_callback_caller(node: Node<'_>, source: &[u8], language: Language) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "function_expression" | "arrow_function")
            && let Some(caller) = javascript_callback_context(parent, source, language)
        {
            return Some(caller);
        }
        current = parent.parent();
    }

    None
}

fn javascript_callback_context(
    function_node: Node<'_>,
    source: &[u8],
    language: Language,
) -> Option<String> {
    let mut current = function_node.parent();
    while let Some(parent) = current {
        if parent.kind() == "call_expression" {
            let target_node = parent
                .child_by_field_name("function")
                .or_else(|| parent.child_by_field_name("name"))
                .or_else(|| parent.child(0))?;
            if target_node.start_byte() <= function_node.start_byte()
                && function_node.end_byte() <= target_node.end_byte()
            {
                return None;
            }

            let raw_target = target_node.utf8_text(source).ok()?;
            let callee = normalize_callee(raw_target, language)?;
            return Some(format!("{callee}.<callback>"));
        }

        current = parent.parent();
    }

    None
}

fn visit_dependency_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
    dependencies: &mut Vec<Dependency>,
) {
    dependencies.extend(dependencies_from_node(
        node,
        source,
        language,
        source_file,
        javascript_bindings,
    ));

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        visit_dependency_node(
            child,
            source,
            language,
            source_file,
            javascript_bindings,
            dependencies,
        );
    }
}

fn dependencies_from_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    match language {
        Language::Bash => bash_dependencies(node, source, language, source_file),
        Language::C | Language::Cpp => c_like_dependencies(node, source, language, source_file),
        Language::CSharp => csharp_dependencies(node, source, language, source_file),
        Language::Php => php_dependencies(node, source, language, source_file),
        Language::Python => python_dependencies(node, source, language, source_file),
        Language::Ruby => ruby_dependencies(node, source, language, source_file),
        Language::Go => go_dependencies(node, source, language, source_file),
        Language::Java => java_dependencies(node, source, language, source_file),
        Language::Rust => rust_dependencies(node, source, language, source_file),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_like_dependencies(node, source, language, source_file, javascript_bindings)
        }
    }
}

fn visit_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    file: &str,
    scope: &mut Vec<String>,
    symbols: &mut Vec<Symbol>,
) {
    if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) && node.kind() == "variable_declarator"
    {
        for (name, kind) in javascript_variable_declarator_symbols(node, source) {
            symbols.push(Symbol {
                name: name.clone(),
                qualified_name: if scope.is_empty() {
                    name
                } else {
                    format!("{}.{}", scope.join("."), name)
                },
                kind,
                language,
                file: file.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
            });
        }
    }

    if let Some((name, kind)) = symbol_from_node(node, source, language) {
        let qualified_name = if scope.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", scope.join("."), name)
        };
        let symbol = Symbol {
            name: name.clone(),
            qualified_name,
            kind,
            language,
            file: file.to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        };
        let should_scope = matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface
        );
        symbols.push(symbol);
        if should_scope {
            scope.push(name);
            visit_children(node, source, language, file, scope, symbols);
            scope.pop();
            return;
        }
    }

    visit_children(node, source, language, file, scope, symbols);
}

fn bash_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    if node.kind() != "command" {
        return Vec::new();
    }

    let text = node.utf8_text(source).unwrap_or_default();
    let Some(target) = bash_source_target(text) else {
        return Vec::new();
    };

    vec![Dependency {
        source_file: source_file.to_string(),
        target,
        resolved_file: None,
        local_alias: None,
        imported_symbol: None,
        kind: "source".to_string(),
        language,
        line: node.start_position().row + 1,
    }]
}

fn bash_source_target(text: &str) -> Option<String> {
    let mut parts = text.split_whitespace();
    let command = parts.next()?.trim();
    if command != "source" && command != "." {
        return None;
    }

    let target = parts
        .find(|part| !part.starts_with('-'))?
        .trim_matches(|ch| matches!(ch, '"' | '\''));
    if target.is_empty()
        || target.contains('$')
        || target.contains('*')
        || target.contains('?')
        || target.contains('`')
        || target.starts_with('<')
        || target.starts_with('>')
    {
        return None;
    }

    Some(target.to_string())
}

fn python_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "import_statement" | "import_from_statement" => {
            python_import_dependencies(node, source, language, source_file)
        }
        _ => Vec::new(),
    }
}

fn python_import_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let line = node.start_position().row + 1;
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("from ") {
        let Some((module, imports)) = rest.split_once(" import ") else {
            return rest
                .split_whitespace()
                .next()
                .map(|target| {
                    vec![Dependency {
                        source_file: source_file.to_string(),
                        target: target.to_string(),
                        resolved_file: None,
                        local_alias: None,
                        imported_symbol: None,
                        kind: "import".to_string(),
                        language,
                        line,
                    }]
                })
                .unwrap_or_default();
        };
        let module = module.trim();
        if module.is_empty() {
            return Vec::new();
        }

        let mut dependencies = vec![Dependency {
            source_file: source_file.to_string(),
            target: module.to_string(),
            resolved_file: None,
            local_alias: None,
            imported_symbol: None,
            kind: "import".to_string(),
            language,
            line,
        }];
        dependencies.extend(python_from_import_aliases(imports).into_iter().map(
            |(imported_symbol, local_alias)| Dependency {
                source_file: source_file.to_string(),
                target: python_join_import_target(module, &imported_symbol),
                resolved_file: None,
                local_alias: Some(local_alias),
                imported_symbol: Some(imported_symbol),
                kind: "import".to_string(),
                language,
                line,
            },
        ));
        return dependencies;
    }

    trimmed
        .strip_prefix("import ")
        .map(|rest| {
            python_import_aliases(rest)
                .into_iter()
                .map(|(target, local_alias)| Dependency {
                    source_file: source_file.to_string(),
                    target,
                    resolved_file: None,
                    local_alias,
                    imported_symbol: Some("*".to_string()),
                    kind: "import".to_string(),
                    language,
                    line,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn go_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "import_declaration" => go_import_dependencies(node, source, language, source_file),
        _ => Vec::new(),
    }
}

fn go_import_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let line = node.start_position().row + 1;
    go_import_specs(text)
        .into_iter()
        .map(|(target, local_alias)| Dependency {
            source_file: source_file.to_string(),
            resolved_file: None,
            target,
            local_alias,
            imported_symbol: Some("*".to_string()),
            kind: "import".to_string(),
            language,
            line,
        })
        .collect()
}

fn java_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "import_declaration" => java_import_dependencies(node, source, language, source_file),
        "package_declaration" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "package",
            java_package_targets,
        ),
        "class_declaration" | "interface_declaration" | "record_declaration" => {
            type_relation_dependencies(
                node,
                source,
                language,
                source_file,
                &["extends", "implements"],
            )
        }
        _ => Vec::new(),
    }
}

fn java_import_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    java_import_target(text)
        .into_iter()
        .map(|(target, kind)| {
            let (local_alias, imported_symbol) = java_import_alias(&target, kind);
            Dependency {
                source_file: source_file.to_string(),
                resolved_file: None,
                target,
                local_alias,
                imported_symbol,
                kind: kind.to_string(),
                language,
                line: node.start_position().row + 1,
            }
        })
        .collect()
}

fn c_like_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "preproc_include" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "include",
            c_include_targets,
        ),
        _ => Vec::new(),
    }
}

fn csharp_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "using_directive" => csharp_using_dependencies(node, source, language, source_file),
        "namespace_declaration" | "file_scoped_namespace_declaration" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "namespace",
            csharp_namespace_targets,
        ),
        "class_declaration" | "record_declaration" => {
            csharp_base_type_dependencies(node, source, language, source_file)
        }
        "property_declaration" => {
            csharp_property_type_dependencies(node, source, language, source_file)
        }
        "method_declaration" => {
            csharp_extension_method_dependencies(node, source, language, source_file)
        }
        "variable_declaration" | "parameter" => {
            csharp_type_binding_dependencies(node, source, language, source_file)
        }
        _ => Vec::new(),
    }
}

fn csharp_using_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    csharp_using_target(text)
        .into_iter()
        .map(|(target, kind)| {
            let (local_alias, imported_symbol) = csharp_using_alias(text, &target, kind);
            Dependency {
                source_file: source_file.to_string(),
                resolved_file: None,
                target,
                local_alias,
                imported_symbol,
                kind: kind.to_string(),
                language,
                line: node.start_position().row + 1,
            }
        })
        .collect()
}

fn csharp_type_binding_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    csharp_type_bindings(text)
        .into_iter()
        .map(|(target, local_alias, wrapper_member)| Dependency {
            source_file: source_file.to_string(),
            resolved_file: None,
            target,
            local_alias: Some(local_alias),
            imported_symbol: wrapper_member.or_else(|| Some("*".to_string())),
            kind: "type_binding".to_string(),
            language,
            line: node.start_position().row + 1,
        })
        .collect()
}

fn csharp_extension_method_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let Some(method) = child_text(node, "name", source) else {
        return Vec::new();
    };
    let text = node.utf8_text(source).unwrap_or_default();
    let Some(target) = csharp_extension_receiver_type(text) else {
        return Vec::new();
    };

    vec![Dependency {
        source_file: source_file.to_string(),
        resolved_file: None,
        target,
        local_alias: Some(method),
        imported_symbol: None,
        kind: "extension_method".to_string(),
        language,
        line: node.start_position().row + 1,
    }]
}

fn csharp_base_type_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let Some(local_alias) = child_text(node, "name", source) else {
        return Vec::new();
    };
    let text = node.utf8_text(source).unwrap_or_default();
    csharp_direct_base_type(text)
        .into_iter()
        .map(|target| Dependency {
            source_file: source_file.to_string(),
            resolved_file: None,
            target,
            local_alias: Some(local_alias.clone()),
            imported_symbol: Some("base".to_string()),
            kind: "base_type".to_string(),
            language,
            line: node.start_position().row + 1,
        })
        .collect()
}

fn csharp_property_type_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let Some(owner) = csharp_containing_type_name(node, source) else {
        return Vec::new();
    };
    let Some(property) = child_text(node, "name", source) else {
        return Vec::new();
    };
    let Some(raw_type) = child_text(node, "type", source) else {
        return Vec::new();
    };
    let Some(target) = clean_csharp_type_name(&raw_type).map(ToOwned::to_owned) else {
        return Vec::new();
    };

    vec![Dependency {
        source_file: source_file.to_string(),
        resolved_file: None,
        target,
        local_alias: Some(property),
        imported_symbol: Some(owner),
        kind: "property_type".to_string(),
        language,
        line: node.start_position().row + 1,
    }]
}

fn csharp_containing_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(
            parent.kind(),
            "class_declaration"
                | "record_declaration"
                | "struct_declaration"
                | "interface_declaration"
        ) {
            return child_text(parent, "name", source);
        }
        current = parent.parent();
    }
    None
}

fn php_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "namespace_use_declaration" => php_use_dependencies(node, source, language, source_file),
        "class_declaration" | "interface_declaration" => type_relation_dependencies(
            node,
            source,
            language,
            source_file,
            &["extends", "implements"],
        ),
        _ => Vec::new(),
    }
}

fn php_use_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let line = node.start_position().row + 1;
    php_use_entries(text)
        .into_iter()
        .map(|(target, local_alias, imported_symbol)| Dependency {
            source_file: source_file.to_string(),
            target,
            resolved_file: None,
            local_alias,
            imported_symbol,
            kind: "use".to_string(),
            language,
            line,
        })
        .collect()
}

fn ruby_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    if node.kind() == "class" {
        return ruby_base_type_dependencies(node, source, language, source_file);
    }

    if node.kind() != "call" {
        return Vec::new();
    }
    let Some(method) = child_text(node, "method", source) else {
        return Vec::new();
    };
    if !matches!(method.as_str(), "require" | "require_relative") {
        return Vec::new();
    }

    ruby_string_argument_targets(node, source)
        .into_iter()
        .map(|target| {
            let (local_alias, imported_symbol) = ruby_require_alias(&target, &method);
            Dependency {
                source_file: source_file.to_string(),
                target,
                resolved_file: None,
                local_alias,
                imported_symbol,
                kind: method.clone(),
                language,
                line: node.start_position().row + 1,
            }
        })
        .collect()
}

fn ruby_base_type_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let Some(local_alias) = child_text(node, "name", source) else {
        return Vec::new();
    };
    let Some(raw_target) =
        child_text(node, "superclass", source).or_else(|| ruby_superclass_from_text(node, source))
    else {
        return Vec::new();
    };
    let Some(target) = clean_type_relation_target(&raw_target) else {
        return Vec::new();
    };

    vec![Dependency {
        source_file: source_file.to_string(),
        target,
        resolved_file: None,
        local_alias: Some(local_alias),
        imported_symbol: Some("extends".to_string()),
        kind: "base_type".to_string(),
        language,
        line: node.start_position().row + 1,
    }]
}

fn ruby_superclass_from_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?;
    let header = text.lines().next()?.trim();
    let (_, raw_target) = header.split_once('<')?;
    let target = raw_target
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim();
    (!target.is_empty()).then(|| target.to_string())
}

fn ruby_require_alias(target: &str, kind: &str) -> (Option<String>, Option<String>) {
    if kind != "require_relative" {
        return (None, None);
    }

    let Some(stem) = Path::new(target).file_stem().and_then(|stem| stem.to_str()) else {
        return (None, None);
    };
    let Some(alias) = ruby_constant_alias(stem) else {
        return (None, None);
    };

    (Some(alias), Some("*".to_string()))
}

fn ruby_constant_alias(stem: &str) -> Option<String> {
    let mut alias = String::new();
    for part in stem
        .split(['_', '-'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        alias.extend(first.to_uppercase());
        alias.push_str(chars.as_str());
    }

    if alias.is_empty() { None } else { Some(alias) }
}

fn rust_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "use_declaration" => rust_use_dependencies(node, source, language, source_file),
        "mod_item" => {
            text_dependencies(node, source, language, source_file, "mod", rust_mod_targets)
        }
        "impl_item" => rust_impl_type_relation_dependencies(node, source, language, source_file),
        _ => Vec::new(),
    }
}

fn rust_impl_type_relation_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let Some((target, local_alias)) = rust_impl_type_relation(text) else {
        return Vec::new();
    };

    vec![Dependency {
        source_file: source_file.to_string(),
        resolved_file: None,
        target,
        local_alias: Some(local_alias),
        imported_symbol: Some("implements".to_string()),
        kind: "base_type".to_string(),
        language,
        line: node.start_position().row + 1,
    }]
}

fn rust_use_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let line = node.start_position().row + 1;
    rust_use_entries(text)
        .into_iter()
        .map(|(target, explicit_alias)| {
            let (local_alias, imported_symbol) = rust_use_alias(&target, explicit_alias.as_deref());
            Dependency {
                source_file: source_file.to_string(),
                resolved_file: None,
                target,
                local_alias,
                imported_symbol,
                kind: "use".to_string(),
                language,
                line,
            }
        })
        .collect()
}

fn rust_impl_type_relation(text: &str) -> Option<(String, String)> {
    let header_end = top_level_index(text, '{').unwrap_or(text.len());
    let mut header = text[..header_end].trim();
    header = header.strip_prefix("unsafe ").unwrap_or(header).trim();
    header = header.strip_prefix("impl")?.trim_start();
    if header.starts_with('<') {
        let generic_end = matching_delimiter(header, 0, '<', '>')?;
        header = header[generic_end + 1..].trim_start();
    }

    let for_index = find_rust_impl_for_keyword(header)?;
    let trait_target = clean_type_relation_target(&header[..for_index])?;
    let self_type = clean_type_relation_target(&header[for_index + "for".len()..])?;
    Some((trait_target, self_type))
}

fn find_rust_impl_for_keyword(header: &str) -> Option<usize> {
    let mut angle_depth = 0;
    for (index, ch) in header.char_indices() {
        match ch {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            'f' if angle_depth == 0
                && header[index..].starts_with("for")
                && is_relation_keyword_boundary(header, index, index + "for".len()) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn javascript_like_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    match node.kind() {
        "import_statement" => {
            let mut dependencies = text_dependencies(
                node,
                source,
                language,
                source_file,
                "import",
                string_literal_targets,
            );
            dependencies.extend(javascript_import_alias_dependencies(
                node,
                source,
                language,
                source_file,
            ));
            dependencies
        }
        "export_statement" => {
            let mut dependencies = text_dependencies(
                node,
                source,
                language,
                source_file,
                "import",
                string_literal_targets,
            );
            dependencies.extend(javascript_export_alias_dependencies(
                node,
                source,
                language,
                source_file,
            ));
            dependencies
        }
        "call_expression" => javascript_call_expression_dependencies(
            node,
            source,
            language,
            source_file,
            javascript_bindings,
        ),
        "variable_declarator" => javascript_variable_module_alias_dependencies(
            node,
            source,
            language,
            source_file,
            javascript_bindings,
        ),
        "class_declaration" | "interface_declaration" | "abstract_class_declaration" => {
            type_relation_dependencies(
                node,
                source,
                language,
                source_file,
                &["extends", "implements"],
            )
        }
        _ => Vec::new(),
    }
}

fn type_relation_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    relation_keywords: &[&'static str],
) -> Vec<Dependency> {
    let Some(local_alias) = child_text(node, "name", source) else {
        return Vec::new();
    };
    let text = node.utf8_text(source).unwrap_or_default();
    type_relation_targets(text, relation_keywords)
        .into_iter()
        .map(|(target, relation)| Dependency {
            source_file: source_file.to_string(),
            resolved_file: None,
            target,
            local_alias: Some(local_alias.clone()),
            imported_symbol: Some(relation.to_string()),
            kind: "base_type".to_string(),
            language,
            line: node.start_position().row + 1,
        })
        .collect()
}

fn type_relation_targets(
    text: &str,
    relation_keywords: &[&'static str],
) -> Vec<(String, &'static str)> {
    let header_end = top_level_index(text, '{').unwrap_or(text.len());
    let header = &text[..header_end];
    let positions = relation_keyword_positions(header, relation_keywords);
    let mut targets = Vec::new();

    for (index, (start, relation)) in positions.iter().enumerate() {
        let relation_end = start + relation.len();
        let section_end = positions
            .get(index + 1)
            .map(|(next_start, _)| *next_start)
            .unwrap_or(header.len());
        let section = header[relation_end..section_end].trim();
        for raw_target in split_type_relation_targets(section) {
            if let Some(target) = clean_type_relation_target(raw_target) {
                targets.push((target, *relation));
            }
        }
    }

    targets
}

fn relation_keyword_positions<'a>(
    text: &str,
    relation_keywords: &[&'a str],
) -> Vec<(usize, &'a str)> {
    let mut positions = Vec::new();
    for relation in relation_keywords {
        let mut search_start = 0;
        while let Some(relative_index) = text[search_start..].find(relation) {
            let start = search_start + relative_index;
            let end = start + relation.len();
            if is_relation_keyword_boundary(text, start, end) {
                positions.push((start, *relation));
            }
            search_start = end;
        }
    }
    positions.sort_by_key(|(start, _)| *start);
    positions
}

fn split_type_relation_targets(raw: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in raw.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                parts.push(&raw[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }

    parts.push(&raw[start..]);
    parts
}

fn is_relation_keyword_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    !before.is_some_and(is_type_identifier_char) && !after.is_some_and(is_type_identifier_char)
}

fn is_type_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn clean_type_relation_target(raw: &str) -> Option<String> {
    let mut target = raw
        .trim()
        .trim_end_matches(';')
        .trim_start_matches('<')
        .trim()
        .trim_start_matches("public ")
        .trim_start_matches("private ")
        .trim_start_matches("protected ")
        .trim();
    if target.is_empty() || top_level_index(target, '(').is_some() {
        return None;
    }
    if let Some(open) = top_level_index(target, '<') {
        target = target[..open].trim();
    }
    let target = target.trim_end_matches("[]").trim();
    (!target.is_empty()).then(|| target.to_string())
}

fn text_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    kind: &str,
    extractor: impl Fn(&str) -> Vec<String>,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    extractor(text)
        .into_iter()
        .map(|target| Dependency {
            source_file: source_file.to_string(),
            resolved_file: None,
            target,
            local_alias: None,
            imported_symbol: None,
            kind: kind.to_string(),
            language,
            line: node.start_position().row + 1,
        })
        .collect()
}

fn javascript_import_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let Some(target) = string_literal_targets(text).into_iter().next() else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();

    if let Some(local_alias) = import_default_alias(text) {
        dependencies.push(alias_dependency(
            source_file,
            &target,
            language,
            node.start_position().row + 1,
            "default".to_string(),
            local_alias,
        ));
    }

    if let Some(local_alias) = import_namespace_alias(text) {
        dependencies.push(namespace_dependency(
            source_file,
            &target,
            language,
            node.start_position().row + 1,
            local_alias,
        ));
    }

    let Some(named_imports) = braced_segment(text) else {
        return dependencies;
    };

    dependencies.extend(
        import_named_aliases(named_imports)
            .into_iter()
            .filter(|(imported_symbol, local_alias)| imported_symbol != local_alias)
            .map(|(imported_symbol, local_alias)| {
                alias_dependency(
                    source_file,
                    &target,
                    language,
                    node.start_position().row + 1,
                    imported_symbol,
                    local_alias,
                )
            }),
    );
    dependencies
}

fn javascript_export_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let Some(target) = string_literal_targets(text).into_iter().next() else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();

    if let Some(local_alias) = export_namespace_alias(text) {
        dependencies.push(export_namespace_dependency(
            source_file,
            &target,
            language,
            node.start_position().row + 1,
            local_alias,
        ));
    }

    let Some(named_exports) = braced_segment(text) else {
        return dependencies;
    };

    dependencies.extend(import_named_aliases(named_exports).into_iter().map(
        |(imported_symbol, local_alias)| Dependency {
            source_file: source_file.to_string(),
            target: target.clone(),
            resolved_file: None,
            local_alias: Some(local_alias),
            imported_symbol: Some(imported_symbol),
            kind: "export_alias".to_string(),
            language,
            line: node.start_position().row + 1,
        },
    ));
    dependencies
}

fn javascript_call_expression_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    let mut dependencies =
        text_dependencies(node, source, language, source_file, "import", |text| {
            javascript_call_expression_targets(text, javascript_bindings)
        });
    dependencies.extend(javascript_dynamic_import_callback_alias_dependencies(
        node,
        source,
        language,
        source_file,
        javascript_bindings,
    ));
    dependencies
}

fn javascript_dynamic_import_callback_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    let text = node.utf8_text(source).unwrap_or_default();
    let trimmed = text.trim_start();
    let Some(then_index) = trimmed.find(".then") else {
        return Vec::new();
    };
    if !is_static_dynamic_import(&trimmed[..then_index]) {
        return Vec::new();
    }
    let Some(target) = static_js_module_call_targets(&trimmed[..then_index], javascript_bindings)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };
    let Some(local_alias) = dynamic_import_then_callback_alias(&trimmed[then_index + 5..]) else {
        return Vec::new();
    };

    vec![namespace_dependency(
        source_file,
        &target,
        language,
        node.start_position().row + 1,
        local_alias,
    )]
}

fn export_namespace_alias(raw: &str) -> Option<Option<String>> {
    let specifier = raw.trim().strip_prefix("export")?.trim();
    let mut parts = specifier.split_whitespace();
    if parts.next()? != "*" {
        return None;
    }

    match parts.next()? {
        "from" => Some(None),
        "as" => {
            let alias = parts.next()?;
            if parts.next() == Some("from") && is_js_identifier(alias) {
                Some(Some(alias.to_string()))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn import_default_alias(raw: &str) -> Option<String> {
    let specifier = raw.trim().strip_prefix("import")?.trim();
    let specifier = specifier.strip_prefix("type ").unwrap_or(specifier).trim();
    if specifier.starts_with('*') || specifier.starts_with('{') {
        return None;
    }

    let alias = if let Some(comma) = top_level_char_index(specifier, ',') {
        specifier[..comma].trim()
    } else {
        let mut parts = specifier.split_whitespace();
        let alias = parts.next()?;
        if parts.next()? != "from" {
            return None;
        }
        alias
    };

    if is_js_identifier(alias) {
        Some(alias.to_string())
    } else {
        None
    }
}

fn import_namespace_alias(raw: &str) -> Option<String> {
    let specifier = raw.trim().strip_prefix("import")?.trim();
    let mut parts = specifier.split_whitespace();
    if parts.clone().next() == Some("type") {
        parts.next();
    }
    if parts.next()? != "*" || parts.next()? != "as" {
        return None;
    }

    let alias = parts.next()?;
    if parts.next() == Some("from") && is_js_identifier(alias) {
        Some(alias.to_string())
    } else {
        None
    }
}

fn javascript_require_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    let Some(name_node) = node.child_by_field_name("name").or_else(|| node.child(0)) else {
        return Vec::new();
    };
    let Ok(name) = name_node.utf8_text(source) else {
        return Vec::new();
    };
    let name = name.trim();

    let value = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("right"))
        .and_then(|child| child.utf8_text(source).ok())
        .unwrap_or_default();
    if !value.contains("require") {
        return Vec::new();
    }
    let Some(target) = static_js_module_call_targets(value, javascript_bindings)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };

    if is_js_identifier(name) {
        return vec![namespace_dependency(
            source_file,
            &target,
            language,
            node.start_position().row + 1,
            name.to_string(),
        )];
    }

    if !name.starts_with('{') {
        return Vec::new();
    }
    let Some(bindings) = braced_segment(name) else {
        return Vec::new();
    };

    object_pattern_aliases(bindings)
        .into_iter()
        .filter(|(imported_symbol, local_alias)| imported_symbol != local_alias)
        .map(|(imported_symbol, local_alias)| {
            alias_dependency(
                source_file,
                &target,
                language,
                node.start_position().row + 1,
                imported_symbol,
                local_alias,
            )
        })
        .collect()
}

fn javascript_variable_module_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    let mut dependencies = javascript_require_alias_dependencies(
        node,
        source,
        language,
        source_file,
        javascript_bindings,
    );
    dependencies.extend(javascript_dynamic_import_alias_dependencies(
        node,
        source,
        language,
        source_file,
        javascript_bindings,
    ));
    dependencies
}

fn javascript_dynamic_import_alias_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<Dependency> {
    let Some(name_node) = node.child_by_field_name("name").or_else(|| node.child(0)) else {
        return Vec::new();
    };
    let Ok(name) = name_node.utf8_text(source) else {
        return Vec::new();
    };
    let name = name.trim();
    if !is_js_identifier(name) {
        return Vec::new();
    }

    let value = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("right"))
        .and_then(|child| child.utf8_text(source).ok())
        .unwrap_or_default();
    if !is_static_dynamic_import(value) {
        return Vec::new();
    }
    let Some(target) = static_js_module_call_targets(value, javascript_bindings)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };

    vec![namespace_dependency(
        source_file,
        &target,
        language,
        node.start_position().row + 1,
        name.to_string(),
    )]
}

fn is_static_dynamic_import(value: &str) -> bool {
    let trimmed = value.trim();
    let expression = trimmed
        .strip_prefix("await ")
        .map(str::trim_start)
        .unwrap_or(trimmed);
    let Some(rest) = expression.strip_prefix("import") else {
        return false;
    };
    rest.trim_start().starts_with('(')
}

fn dynamic_import_then_callback_alias(raw_after_then: &str) -> Option<String> {
    let rest = raw_after_then.trim_start().strip_prefix('(')?.trim_start();
    let callback = strip_async_callback_prefix(rest);

    if callback.starts_with('(') {
        let close = matching_delimiter(callback, 0, '(', ')')?;
        let alias = callback[1..close].trim();
        let after_params = callback[close + 1..].trim_start();
        if is_js_identifier(alias) && after_params.starts_with("=>") {
            return Some(alias.to_string());
        }
        return None;
    }

    let alias_end = callback
        .char_indices()
        .find_map(|(index, ch)| {
            if ch.is_whitespace() || matches!(ch, ',' | '=') {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(callback.len());
    let alias = callback[..alias_end].trim();
    let after_alias = callback[alias_end..].trim_start();
    if is_js_identifier(alias) && after_alias.starts_with("=>") {
        Some(alias.to_string())
    } else {
        None
    }
}

fn strip_async_callback_prefix(raw: &str) -> &str {
    let Some(rest) = raw.strip_prefix("async") else {
        return raw;
    };
    if rest.chars().next().is_some_and(char::is_whitespace) {
        rest.trim_start()
    } else {
        raw
    }
}

fn javascript_static_string_bindings(root: Node<'_>, source: &[u8]) -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    collect_javascript_static_string_bindings(root, source, &mut bindings);
    bindings
}

fn collect_javascript_static_string_bindings(
    node: Node<'_>,
    source: &[u8],
    bindings: &mut HashMap<String, String>,
) {
    if node.kind() == "variable_declarator"
        && let Some((name, value)) = javascript_static_string_binding(node, source, bindings)
    {
        bindings.insert(name, value);
    }

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        collect_javascript_static_string_bindings(child, source, bindings);
    }
}

fn javascript_static_string_binding(
    node: Node<'_>,
    source: &[u8],
    bindings: &HashMap<String, String>,
) -> Option<(String, String)> {
    let name_node = node.child_by_field_name("name").or_else(|| node.child(0))?;
    let name = name_node.utf8_text(source).ok()?.trim();
    if !is_js_identifier(name) {
        return None;
    }

    let value = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("right"))
        .and_then(|child| child.utf8_text(source).ok())?;
    let value = javascript_static_string_expression_value(value, Some(bindings))?;

    Some((name.to_string(), value))
}

fn javascript_call_expression_targets(
    text: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let static_targets = static_js_module_call_targets(text, javascript_bindings);
    if static_targets.is_empty() {
        string_literal_targets(text)
    } else {
        static_targets
    }
}

fn static_js_module_call_targets(
    text: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let mut targets = Vec::new();
    targets.extend(static_js_module_call_targets_for_keyword(
        text,
        "require",
        javascript_bindings,
    ));
    targets.extend(static_js_module_call_targets_for_keyword(
        text,
        "import",
        javascript_bindings,
    ));
    targets
}

fn static_js_module_call_targets_for_keyword(
    text: &str,
    keyword: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let mut targets = Vec::new();
    let mut search_start = 0;
    while let Some(relative_start) = text[search_start..].find(keyword) {
        let keyword_start = search_start + relative_start;
        let after_keyword = keyword_start + keyword.len();
        if !has_js_identifier_boundaries(text, keyword_start, after_keyword) {
            search_start = after_keyword;
            continue;
        }

        let open = skip_ascii_whitespace(text, after_keyword);
        if !text[open..].starts_with('(') {
            search_start = after_keyword;
            continue;
        }

        let Some(close) = matching_delimiter(text, open, '(', ')') else {
            search_start = open + 1;
            continue;
        };
        if let Some(target) =
            javascript_static_string_expression_value(&text[open + 1..close], javascript_bindings)
        {
            targets.push(target);
        }
        search_start = close + 1;
    }
    targets
}

fn javascript_static_string_expression_value(
    raw: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
) -> Option<String> {
    let mut seen = HashSet::new();
    javascript_static_string_expression_value_inner(raw, javascript_bindings, &mut seen)
}

fn javascript_static_string_expression_value_inner(
    raw: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
    seen: &mut HashSet<String>,
) -> Option<String> {
    let first_arg = top_level_char_index(raw, ',')
        .map(|comma| &raw[..comma])
        .unwrap_or(raw)
        .trim();
    if first_arg.is_empty() {
        return None;
    }

    javascript_static_string_operand_value(first_arg, javascript_bindings, seen)
}

fn javascript_static_string_operand_value(
    raw: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
    seen: &mut HashSet<String>,
) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let plus_indices = top_level_char_indices(raw, '+');
    if !plus_indices.is_empty() {
        let mut value = String::new();
        let mut start = 0;
        for plus in plus_indices {
            value.push_str(&javascript_static_string_operand_value(
                &raw[start..plus],
                javascript_bindings,
                seen,
            )?);
            start = plus + 1;
        }
        value.push_str(&javascript_static_string_operand_value(
            &raw[start..],
            javascript_bindings,
            seen,
        )?);
        return Some(value);
    }

    if let Some(value) = string_literal_value(raw) {
        return Some(value);
    }

    if let Some(value) = javascript_template_literal_value(raw, javascript_bindings, seen) {
        return Some(value);
    }

    if let Some(bindings) = javascript_bindings
        && is_js_identifier(raw)
        && let Some(value) = bindings.get(raw)
    {
        if !seen.insert(raw.to_string()) {
            return None;
        }
        seen.remove(raw);
        return Some(value.clone());
    }

    None
}

fn javascript_template_literal_value(
    raw: &str,
    javascript_bindings: Option<&HashMap<String, String>>,
    seen: &mut HashSet<String>,
) -> Option<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('`') || !trimmed.ends_with('`') || trimmed.len() < 2 {
        return None;
    }

    let body = &trimmed[1..trimmed.len() - 1];
    let mut value = String::new();
    let mut start = 0;

    while let Some(relative_start) = body[start..].find("${") {
        let expression_start = start + relative_start;
        value.push_str(&body[start..expression_start]);

        let open = expression_start + 1;
        let close = matching_delimiter(body, open, '{', '}')?;
        let expression = &body[expression_start + 2..close];
        value.push_str(&javascript_static_string_expression_value_inner(
            expression,
            javascript_bindings,
            seen,
        )?);
        start = close + 1;
    }

    value.push_str(&body[start..]);
    Some(value)
}

fn has_js_identifier_boundaries(raw: &str, start: usize, end: usize) -> bool {
    !raw[..start]
        .chars()
        .next_back()
        .is_some_and(is_js_identifier_part)
        && !raw[end..].chars().next().is_some_and(is_js_identifier_part)
}

fn is_js_identifier_part(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn skip_ascii_whitespace(raw: &str, mut index: usize) -> usize {
    while index < raw.len() {
        let ch = raw[index..].chars().next().expect("valid char boundary");
        if !ch.is_ascii_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn alias_dependency(
    source_file: &str,
    target: &str,
    language: Language,
    line: usize,
    imported_symbol: String,
    local_alias: String,
) -> Dependency {
    Dependency {
        source_file: source_file.to_string(),
        target: target.to_string(),
        resolved_file: None,
        local_alias: Some(local_alias),
        imported_symbol: Some(imported_symbol),
        kind: "import_alias".to_string(),
        language,
        line,
    }
}

fn namespace_dependency(
    source_file: &str,
    target: &str,
    language: Language,
    line: usize,
    local_alias: String,
) -> Dependency {
    Dependency {
        source_file: source_file.to_string(),
        target: target.to_string(),
        resolved_file: None,
        local_alias: Some(local_alias),
        imported_symbol: Some("*".to_string()),
        kind: "import_namespace".to_string(),
        language,
        line,
    }
}

fn export_namespace_dependency(
    source_file: &str,
    target: &str,
    language: Language,
    line: usize,
    local_alias: Option<String>,
) -> Dependency {
    Dependency {
        source_file: source_file.to_string(),
        target: target.to_string(),
        resolved_file: None,
        local_alias,
        imported_symbol: Some("*".to_string()),
        kind: "export_namespace".to_string(),
        language,
        line,
    }
}

fn braced_segment(raw: &str) -> Option<&str> {
    let open = raw.find('{')?;
    let close = matching_delimiter(raw, open, '{', '}')?;
    Some(&raw[open + 1..close])
}

fn import_named_aliases(raw: &str) -> Vec<(String, String)> {
    split_top_level_commas(raw)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.trim().strip_prefix("type ").unwrap_or(entry.trim());
            if entry.is_empty() {
                return None;
            }

            let parts = entry.split_whitespace().collect::<Vec<_>>();
            match parts.as_slice() {
                [imported, "as", local] => clean_js_property_key(imported)
                    .filter(|_| is_js_identifier(local))
                    .map(|imported| (imported.to_string(), (*local).to_string())),
                [name] => clean_js_property_key(name)
                    .filter(|_| is_js_identifier(name))
                    .map(|name| (name.to_string(), name.to_string())),
                _ => None,
            }
        })
        .collect()
}

fn object_pattern_aliases(raw: &str) -> Vec<(String, String)> {
    split_top_level_commas(raw)
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() || entry.starts_with("...") {
                return None;
            }

            if let Some(colon) = top_level_char_index(entry, ':') {
                let imported = clean_js_property_key(&entry[..colon])?;
                let local = local_binding_identifier(&entry[colon + 1..])?;
                return Some((imported.to_string(), local));
            }

            let local = local_binding_identifier(entry)?;
            Some((local.clone(), local))
        })
        .collect()
}

fn local_binding_identifier(raw: &str) -> Option<String> {
    let without_default = top_level_char_index(raw, '=')
        .map(|equals| &raw[..equals])
        .unwrap_or(raw)
        .trim()
        .trim_start_matches("...")
        .trim();
    if is_js_identifier(without_default) {
        Some(without_default.to_string())
    } else {
        None
    }
}

fn split_top_level_commas(raw: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for index in top_level_char_indices(raw, ',') {
        parts.push(&raw[start..index]);
        start = index + 1;
    }
    parts.push(&raw[start..]);
    parts
}

fn top_level_char_index(raw: &str, needle: char) -> Option<usize> {
    top_level_char_indices(raw, needle).into_iter().next()
}

fn top_level_char_indices(raw: &str, needle: char) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut paren_depth = 0;
    let mut bracket_depth = 0;
    let mut brace_depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in raw.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' if needle != '(' => paren_depth += 1,
            ')' if needle != ')' && paren_depth > 0 => paren_depth -= 1,
            '[' if needle != '[' => bracket_depth += 1,
            ']' if needle != ']' && bracket_depth > 0 => bracket_depth -= 1,
            '{' if needle != '{' => brace_depth += 1,
            '}' if needle != '}' && brace_depth > 0 => brace_depth -= 1,
            _ => {}
        }

        if ch == needle && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            indices.push(index);
        }
    }

    indices
}

fn matching_delimiter(raw: &str, open_index: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in raw.char_indices().filter(|(index, _)| *index >= open_index) {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            ch if ch == open => depth += 1,
            ch if ch == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn resolve_dependencies(
    root: &Path,
    dependencies: &mut [Dependency],
    package_conditions: &[String],
) {
    for dependency in dependencies {
        dependency.resolved_file = resolve_dependency(root, dependency, package_conditions);
    }
}

fn resolve_dependency(
    root: &Path,
    dependency: &Dependency,
    package_conditions: &[String],
) -> Option<String> {
    match dependency.language {
        Language::Bash => resolve_bash_target(root, dependency),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            resolve_javascript_like_target(root, dependency, package_conditions)
        }
        Language::C | Language::Cpp => resolve_c_like_target(root, dependency),
        Language::CSharp => resolve_csharp_target(root, dependency),
        Language::Go => resolve_go_target(root, dependency),
        Language::Java => resolve_java_target(root, dependency),
        Language::Python => resolve_python_target(root, dependency),
        Language::Ruby => resolve_ruby_target(root, dependency),
        Language::Rust => resolve_rust_target(root, dependency),
        Language::Php => resolve_php_target(root, dependency),
    }
}

fn resolve_bash_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.kind != "source" {
        return None;
    }

    const EXTENSIONS: &[&str] = &["sh", "bash"];
    if let Some(resolved) = resolve_relative_target(
        root,
        &dependency.source_file,
        &dependency.target,
        EXTENSIONS,
    ) {
        return Some(resolved);
    }

    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""));
    if let Some(resolved) = resolve_base(root, source_dir.join(&dependency.target), EXTENSIONS) {
        return Some(resolved);
    }

    if dependency.target.contains('/') {
        return resolve_base(root, PathBuf::from(&dependency.target), EXTENSIONS);
    }

    None
}

fn resolve_go_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let go_mod_path = find_nearest_go_mod(root, &dependency.source_file)?;
    let module_path = go_module_path(root, &go_mod_path)?;
    let package_suffix = dependency.target.strip_prefix(&module_path)?;
    if !package_suffix.is_empty() && !package_suffix.starts_with('/') {
        return None;
    }

    let module_dir = go_mod_path.parent().unwrap_or(Path::new(""));
    let package_dir = module_dir.join(package_suffix.trim_start_matches('/'));
    resolve_go_package_dir(root, package_dir)
}

fn find_nearest_go_mod(root: &Path, source_file: &str) -> Option<PathBuf> {
    let mut current = Path::new(source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    loop {
        let candidate = current.join("go.mod");
        if root.join(&candidate).is_file() {
            return Some(candidate);
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn go_module_path(root: &Path, go_mod_path: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join(go_mod_path)).ok()?;
    parse_go_module_path(&text)
}

fn parse_go_module_path(text: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line
            .split_once("//")
            .map(|(before, _)| before)
            .unwrap_or(raw_line)
            .trim();
        let Some(module_path) = line.strip_prefix("module") else {
            continue;
        };
        if !module_path.chars().next().is_some_and(char::is_whitespace) {
            continue;
        }
        let module_path = module_path.trim().trim_matches(['"', '`']);
        if !module_path.is_empty() {
            return Some(module_path.to_string());
        }
    }
    None
}

fn resolve_go_package_dir(root: &Path, package_dir: PathBuf) -> Option<String> {
    let mut candidates = fs::read_dir(root.join(&package_dir))
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_file())
                .and_then(|_| {
                    let file_name = entry.file_name();
                    let file_name_text = file_name.to_string_lossy();
                    file_name_text
                        .ends_with(".go")
                        .then(|| package_dir.join(file_name))
                })
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        let file_name = candidate
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        (
            file_name == "doc.go",
            file_name.ends_with("_test.go"),
            candidate.clone(),
        )
    });
    existing_relative(root, candidates)
}

fn resolve_csharp_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if !matches!(
        dependency.kind.as_str(),
        "using" | "using_alias" | "using_static"
    ) || csharp_external_target(&dependency.target)
    {
        return None;
    }

    let target_path = dependency.target.replace('.', "/");
    let roots = csharp_source_roots(&dependency.source_file);
    let exact_candidates = roots
        .iter()
        .map(|source_root| source_root.join(&target_path).with_extension("cs"))
        .collect::<Vec<_>>();
    if let Some(resolved) = existing_relative(root, exact_candidates) {
        return Some(resolved);
    }

    for source_root in roots {
        if let Some(resolved) = first_csharp_file_in_dir(root, source_root.join(&target_path)) {
            return Some(resolved);
        }
    }
    None
}

fn csharp_external_target(target: &str) -> bool {
    target == "System"
        || target.starts_with("System.")
        || target == "Microsoft"
        || target.starts_with("Microsoft.")
}

fn csharp_source_roots(source_file: &str) -> Vec<PathBuf> {
    let normalized = source_file.replace('\\', "/");
    let mut roots = Vec::new();

    for marker in ["src", "test", "tests"] {
        let marker_with_slash = format!("{marker}/");
        if normalized.starts_with(&marker_with_slash) {
            roots.push(PathBuf::from(marker));
        }

        let nested_marker = format!("/{marker}/");
        if let Some(index) = normalized.find(&nested_marker) {
            roots.push(PathBuf::from(&normalized[..index]).join(marker));
        }
    }

    roots.extend([PathBuf::from("src"), PathBuf::new()]);
    roots.sort();
    roots.dedup();
    roots
}

fn first_csharp_file_in_dir(root: &Path, dir: PathBuf) -> Option<String> {
    let mut candidates = fs::read_dir(root.join(&dir))
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_file())
                .and_then(|_| {
                    let file_name = entry.file_name();
                    let file_name_text = file_name.to_string_lossy();
                    file_name_text.ends_with(".cs").then(|| dir.join(file_name))
                })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    existing_relative(root, candidates)
}

fn resolve_java_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if !matches!(dependency.kind.as_str(), "import" | "import_static")
        || dependency.target.ends_with(".*")
    {
        return None;
    }

    let import_paths =
        java_import_candidate_paths(&dependency.target, dependency.kind == "import_static");
    if import_paths.is_empty() {
        return None;
    }

    let candidates = java_source_roots(&dependency.source_file)
        .into_iter()
        .flat_map(|source_root| {
            import_paths
                .iter()
                .map(move |import_path| source_root.join(import_path))
        })
        .collect::<Vec<_>>();
    existing_relative(root, candidates)
}

fn java_import_candidate_paths(target: &str, allow_member_fallback: bool) -> Vec<PathBuf> {
    let segments = target
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Vec::new();
    }

    if !allow_member_fallback {
        return vec![PathBuf::from(segments.join("/")).with_extension("java")];
    }

    (1..=segments.len())
        .rev()
        .map(|len| PathBuf::from(segments[..len].join("/")).with_extension("java"))
        .collect()
}

fn java_source_roots(source_file: &str) -> Vec<PathBuf> {
    let normalized = source_file.replace('\\', "/");
    let mut roots = Vec::new();

    for marker in [
        "src/main/java",
        "src/test/java",
        "src/integrationTest/java",
        "src/androidTest/java",
        "src",
    ] {
        let marker_with_slash = format!("{marker}/");
        if normalized.starts_with(&marker_with_slash) {
            roots.push(PathBuf::from(marker));
        }

        let nested_marker = format!("/{marker}/");
        if let Some(index) = normalized.find(&nested_marker) {
            roots.push(PathBuf::from(&normalized[..index]).join(marker));
        }
    }

    roots.extend([
        PathBuf::from("src/main/java"),
        PathBuf::from("src/test/java"),
        PathBuf::from("src"),
        PathBuf::new(),
    ]);
    roots.sort();
    roots.dedup();
    roots
}

fn resolve_php_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.kind == "base_type" {
        return resolve_php_type_relation_target(root, dependency);
    }

    if dependency.kind != "use" {
        return None;
    }

    let import_paths = php_use_candidate_paths(&dependency.target);
    if import_paths.is_empty() {
        return None;
    }

    let candidates = php_source_roots(&dependency.source_file)
        .into_iter()
        .flat_map(|source_root| {
            import_paths
                .iter()
                .map(move |import_path| source_root.join(import_path))
        })
        .collect::<Vec<_>>();
    existing_relative(root, candidates)
}

fn resolve_php_type_relation_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let import_paths = php_use_candidate_paths(&dependency.target);
    if import_paths.is_empty() {
        return None;
    }

    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""));
    let mut candidates = import_paths
        .iter()
        .map(|import_path| source_dir.join(import_path))
        .collect::<Vec<_>>();
    candidates.extend(
        php_source_roots(&dependency.source_file)
            .into_iter()
            .flat_map(|source_root| {
                import_paths
                    .iter()
                    .map(move |import_path| source_root.join(import_path))
            }),
    );

    existing_relative(root, candidates)
}

fn php_use_candidate_paths(target: &str) -> Vec<PathBuf> {
    let normalized = target
        .trim_start_matches('\\')
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut candidates = vec![PathBuf::from(&normalized).with_extension("php")];
    if let Some(app_relative) = normalized.strip_prefix("App/") {
        candidates.push(PathBuf::from(app_relative).with_extension("php"));
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn php_source_roots(source_file: &str) -> Vec<PathBuf> {
    let normalized = source_file.replace('\\', "/");
    let mut roots = Vec::new();

    for marker in ["src", "app", "lib"] {
        let marker_with_slash = format!("{marker}/");
        if normalized.starts_with(&marker_with_slash) {
            roots.push(PathBuf::from(marker));
        }

        let nested_marker = format!("/{marker}/");
        if let Some(index) = normalized.find(&nested_marker) {
            roots.push(PathBuf::from(&normalized[..index]).join(marker));
        }
    }

    roots.extend([
        PathBuf::from("src"),
        PathBuf::from("app"),
        PathBuf::from("lib"),
        PathBuf::new(),
    ]);
    roots.sort();
    roots.dedup();
    roots
}

fn resolve_python_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.target.starts_with('.') {
        resolve_python_relative_target(root, dependency)
    } else {
        resolve_python_absolute_target(root, dependency)
    }
}

fn resolve_python_absolute_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let target_base = PathBuf::from(dependency.target.replace('.', "/"));
    for candidate in python_target_candidates(target_base) {
        if let Some(resolved) = resolve_base(root, candidate, &["py"]) {
            return Some(resolved);
        }
    }
    None
}

fn resolve_python_relative_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let leading_dots = dependency
        .target
        .chars()
        .take_while(|character| *character == '.')
        .count();
    if leading_dots == 0 {
        return None;
    }

    let mut base = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for _ in 1..leading_dots {
        base.pop();
    }

    let rest = dependency.target[leading_dots..].replace('.', "/");
    let target_base = if rest.is_empty() {
        base
    } else {
        base.join(rest)
    };
    for candidate in python_target_candidates(target_base) {
        if let Some(resolved) = resolve_base(root, candidate, &["py"]) {
            return Some(resolved);
        }
    }
    None
}

fn python_target_candidates(target: PathBuf) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut current = Some(target.as_path());
    while let Some(candidate) = current {
        candidates.push(candidate.to_path_buf());
        current = candidate.parent().filter(|parent| parent != &Path::new(""));
    }
    candidates
}

fn resolve_ruby_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.kind == "base_type" {
        return resolve_ruby_type_relation_target(root, dependency);
    }

    if dependency.kind != "require_relative" {
        return None;
    }

    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""));
    resolve_base(root, source_dir.join(&dependency.target), &["rb"])
}

fn resolve_ruby_type_relation_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""));
    let target_path = ruby_constant_path(&dependency.target)?;
    let mut candidates = vec![source_dir.join(&target_path)];
    candidates.extend(
        ["lib", "app", "src"]
            .into_iter()
            .map(|root_dir| PathBuf::from(root_dir).join(&target_path)),
    );
    existing_relative(
        root,
        candidates
            .into_iter()
            .map(|candidate| candidate.with_extension("rb")),
    )
}

fn ruby_constant_path(target: &str) -> Option<PathBuf> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts = trimmed
        .split("::")
        .map(ruby_constant_segment_to_snake)
        .collect::<Option<Vec<_>>>()?;
    Some(parts.into_iter().collect())
}

fn ruby_constant_segment_to_snake(segment: &str) -> Option<String> {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut output = String::new();
    for (index, ch) in trimmed.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                output.push('_');
            }
            output.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            return None;
        }
    }

    (!output.is_empty()).then_some(output)
}

fn resolve_rust_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.kind == "mod" {
        let source_dir = Path::new(&dependency.source_file)
            .parent()
            .unwrap_or(Path::new(""));
        let direct = source_dir.join(format!("{}.rs", dependency.target));
        let nested = source_dir.join(&dependency.target).join("mod.rs");
        return existing_relative(root, vec![direct, nested]);
    }

    if dependency.kind == "use" {
        return resolve_rust_use_target(root, dependency);
    }

    None
}

fn resolve_rust_use_target(root: &Path, dependency: &Dependency) -> Option<String> {
    let (base, rest) = if let Some(rest) = dependency.target.strip_prefix("crate::") {
        (rust_crate_source_root(&dependency.source_file), rest)
    } else if let Some(rest) = dependency.target.strip_prefix("super::") {
        (rust_super_module_dir(&dependency.source_file), rest)
    } else if let Some(rest) = dependency.target.strip_prefix("self::") {
        (rust_current_module_dir(&dependency.source_file), rest)
    } else {
        return None;
    };

    let candidates = rust_use_path_candidates(base, rest);
    existing_relative(root, candidates)
}

fn rust_crate_source_root(source_file: &str) -> PathBuf {
    let normalized = source_file.replace('\\', "/");
    if normalized.starts_with("src/") || normalized == "src" {
        return PathBuf::from("src");
    }
    if let Some(index) = normalized.find("/src/") {
        return PathBuf::from(&normalized[..index]).join("src");
    }
    PathBuf::new()
}

fn rust_super_module_dir(source_file: &str) -> PathBuf {
    let source_path = Path::new(source_file);
    let source_dir = source_path.parent().unwrap_or(Path::new(""));
    if source_path.file_name().and_then(|name| name.to_str()) == Some("mod.rs") {
        return source_dir.parent().unwrap_or(Path::new("")).to_path_buf();
    }
    source_dir.to_path_buf()
}

fn rust_current_module_dir(source_file: &str) -> PathBuf {
    let source_path = Path::new(source_file);
    let source_dir = source_path.parent().unwrap_or(Path::new(""));
    if source_path.file_name().and_then(|name| name.to_str()) == Some("mod.rs") {
        return source_dir.to_path_buf();
    }
    let Some(stem) = source_path.file_stem() else {
        return source_dir.to_path_buf();
    };
    source_dir.join(stem)
}

fn rust_use_path_candidates(base: PathBuf, target: &str) -> Vec<PathBuf> {
    let segments = target
        .split("::")
        .map(|segment| segment.trim())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for len in (1..=segments.len()).rev() {
        let candidate = base.join(segments[..len].join("/"));
        candidates.push(candidate.with_extension("rs"));
        candidates.push(candidate.join("mod.rs"));
    }
    candidates
}

fn resolve_c_like_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.target.starts_with('<') {
        return None;
    }

    if let Some(resolved) = resolve_relative_target(
        root,
        &dependency.source_file,
        &dependency.target,
        &["h", "hpp", "hh", "hxx", "c", "cc", "cpp", "cxx"],
    ) {
        return Some(resolved);
    }

    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .unwrap_or(Path::new(""));
    if let Some(resolved) = resolve_base(
        root,
        source_dir.join(&dependency.target),
        &["h", "hpp", "hh", "hxx", "c", "cc", "cpp", "cxx"],
    ) {
        return Some(resolved);
    }

    if dependency.target.contains('/') {
        return resolve_base(
            root,
            PathBuf::from(&dependency.target),
            &["h", "hpp", "hh", "hxx", "c", "cc", "cpp", "cxx"],
        );
    }

    None
}

fn resolve_javascript_like_target(
    root: &Path,
    dependency: &Dependency,
    package_conditions: &[String],
) -> Option<String> {
    const EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];
    if let Some(resolved) = resolve_relative_target(
        root,
        &dependency.source_file,
        &dependency.target,
        EXTENSIONS,
    ) {
        return Some(resolved);
    }

    match resolve_package_imports_target(root, dependency, EXTENSIONS, package_conditions) {
        PackageImportResolution::Resolved(resolved) => return Some(resolved),
        PackageImportResolution::Blocked => return None,
        PackageImportResolution::Unresolved => {}
    }

    resolve_tsconfig_target(root, dependency, EXTENSIONS)
        .or_else(|| {
            resolve_package_exports_target(root, dependency, EXTENSIONS, package_conditions)
        })
        .or_else(|| {
            resolve_workspace_package_exports_target(
                root,
                dependency,
                EXTENSIONS,
                package_conditions,
            )
        })
        .or_else(|| {
            resolve_node_modules_package_exports_target(
                root,
                dependency,
                EXTENSIONS,
                package_conditions,
            )
        })
}

enum PackageImportResolution {
    Resolved(String),
    Blocked,
    Unresolved,
}

fn resolve_tsconfig_target(
    root: &Path,
    dependency: &Dependency,
    extensions: &[&str],
) -> Option<String> {
    if dependency.target.starts_with('.') || dependency.target.starts_with('/') {
        return None;
    }

    let config_path = find_javascript_config(root, &dependency.source_file)?;
    let config = load_javascript_config(root, &config_path, &mut HashSet::new())?;

    if let Some(paths) = config.paths.as_ref()
        && let Some(paths_target) = resolve_tsconfig_paths_target(
            root,
            &dependency.target,
            paths,
            &config.base_dir,
            extensions,
        )
    {
        return Some(paths_target);
    }

    resolve_base(root, config.base_dir.join(&dependency.target), extensions)
}

#[derive(Debug)]
struct JavascriptConfig {
    base_dir: PathBuf,
    paths: Option<Value>,
}

fn load_javascript_config(
    root: &Path,
    config_path: &Path,
    seen: &mut HashSet<PathBuf>,
) -> Option<JavascriptConfig> {
    let config_path = normalize_path(config_path);
    if !seen.insert(config_path.clone()) {
        return None;
    }

    let config_text = fs::read_to_string(root.join(&config_path)).ok()?;
    let config: Value = serde_json::from_str(&config_text).ok()?;
    let config_dir = config_path.parent().unwrap_or(Path::new(""));

    let inherited = config
        .get("extends")
        .and_then(Value::as_str)
        .and_then(|extends| resolve_javascript_config_extends(root, config_dir, extends))
        .and_then(|extends_path| load_javascript_config(root, &extends_path, seen));

    let compiler_options = config.get("compilerOptions");
    let base_dir = compiler_options
        .and_then(|options| options.get("baseUrl"))
        .and_then(Value::as_str)
        .map(|base_url| normalize_path(&config_dir.join(base_url)))
        .or_else(|| inherited.as_ref().map(|config| config.base_dir.clone()))
        .unwrap_or_else(|| config_dir.to_path_buf());

    let paths = compiler_options
        .and_then(|options| options.get("paths"))
        .cloned()
        .or_else(|| inherited.and_then(|config| config.paths));

    Some(JavascriptConfig { base_dir, paths })
}

fn resolve_javascript_config_extends(
    root: &Path,
    config_dir: &Path,
    extends: &str,
) -> Option<PathBuf> {
    let extends_path = Path::new(extends);
    if !(extends.starts_with('.') || extends_path.is_absolute()) {
        return None;
    }

    let base = if extends_path.is_absolute() {
        extends_path.strip_prefix(root).ok()?.to_path_buf()
    } else {
        normalize_path(&config_dir.join(extends_path))
    };
    let mut candidates = vec![base.clone()];
    if base.extension().is_none() {
        candidates.push(base.with_extension("json"));
        candidates.push(base.join("tsconfig.json"));
        candidates.push(base.join("jsconfig.json"));
    }

    candidates
        .into_iter()
        .map(|candidate| normalize_path(&candidate))
        .find(|candidate| root.join(candidate).is_file())
}

fn find_javascript_config(root: &Path, source_file: &str) -> Option<PathBuf> {
    let mut current = Path::new(source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    loop {
        for filename in ["tsconfig.json", "jsconfig.json"] {
            let candidate = current.join(filename);
            if root.join(&candidate).is_file() {
                return Some(candidate);
            }
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn resolve_tsconfig_paths_target(
    root: &Path,
    target: &str,
    paths_value: &Value,
    base_dir: &Path,
    extensions: &[&str],
) -> Option<String> {
    let paths = paths_value.as_object()?;
    let (mappings, wildcards) = path_pattern_mapping(paths, target)?;
    let mapping_values = mappings.as_array()?;
    for mapping in mapping_values.iter().filter_map(Value::as_str) {
        let mapped = apply_path_mapping(mapping, &wildcards)?;
        if let Some(resolved) = resolve_base(root, base_dir.join(mapped), extensions) {
            return Some(resolved);
        }
    }
    None
}

fn path_pattern_captures(pattern: &str, target: &str) -> Option<Vec<String>> {
    if !pattern.contains('*') {
        return (pattern == target).then(Vec::new);
    }

    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut captures = Vec::new();
    let prefix = parts[0];
    if !target.starts_with(prefix) {
        return None;
    }
    let mut position = prefix.len();

    for (index, part) in parts.iter().enumerate().skip(1) {
        if index == parts.len() - 1 {
            if !target[position..].ends_with(part) {
                return None;
            }
            let capture_end = target.len().checked_sub(part.len())?;
            if capture_end < position {
                return None;
            }
            captures.push(target[position..capture_end].to_string());
            position = target.len();
            continue;
        }

        let relative_match = target[position..].find(part)?;
        let capture_end = position + relative_match;
        captures.push(target[position..capture_end].to_string());
        position = capture_end + part.len();
    }

    Some(captures)
}

fn apply_path_mapping(mapping: &str, wildcards: &[String]) -> Option<PathBuf> {
    if !mapping.contains('*') {
        return Some(PathBuf::from(mapping));
    }

    let parts = mapping.split('*').collect::<Vec<_>>();
    if parts.len() - 1 != wildcards.len() {
        return None;
    }

    let mut mapped = String::from(parts[0]);
    for (wildcard, suffix) in wildcards.iter().zip(parts.iter().skip(1)) {
        mapped.push_str(wildcard);
        mapped.push_str(suffix);
    }
    Some(PathBuf::from(mapped))
}

fn resolve_package_exports_target(
    root: &Path,
    dependency: &Dependency,
    extensions: &[&str],
    package_conditions: &[String],
) -> Option<String> {
    if dependency.target.starts_with('.') || dependency.target.starts_with('/') {
        return None;
    }

    let package_path = find_package_json(root, &dependency.source_file)?;
    let package_text = fs::read_to_string(root.join(&package_path)).ok()?;
    let package: Value = serde_json::from_str(&package_text).ok()?;
    let name = package.get("name")?.as_str()?;
    let subpath = package_export_subpath(name, &dependency.target)?;
    let package_dir = package_path.parent().unwrap_or(Path::new(""));
    resolve_package_entry(
        root,
        package_dir,
        &package,
        &subpath,
        extensions,
        package_conditions,
    )
}

fn resolve_package_imports_target(
    root: &Path,
    dependency: &Dependency,
    extensions: &[&str],
    package_conditions: &[String],
) -> PackageImportResolution {
    if !dependency.target.starts_with('#') {
        return PackageImportResolution::Unresolved;
    }

    let Some(package_path) = find_package_json(root, &dependency.source_file) else {
        return PackageImportResolution::Unresolved;
    };
    let Ok(package_text) = fs::read_to_string(root.join(&package_path)) else {
        return PackageImportResolution::Unresolved;
    };
    let Ok(package) = serde_json::from_str::<Value>(&package_text) else {
        return PackageImportResolution::Unresolved;
    };
    let package_dir = package_path.parent().unwrap_or(Path::new(""));
    let Some(imports) = package.get("imports") else {
        return PackageImportResolution::Unresolved;
    };
    let Some(mappings) = package_import_mappings(imports, &dependency.target, package_conditions)
    else {
        return PackageImportResolution::Unresolved;
    };
    if mappings.is_empty() {
        return PackageImportResolution::Blocked;
    }
    for mapped in mappings {
        if let Some(resolved) = resolve_base(root, package_dir.join(mapped), extensions) {
            return PackageImportResolution::Resolved(resolved);
        }
    }
    PackageImportResolution::Unresolved
}

fn resolve_node_modules_package_exports_target(
    root: &Path,
    dependency: &Dependency,
    extensions: &[&str],
    package_conditions: &[String],
) -> Option<String> {
    if dependency.target.starts_with('.') || dependency.target.starts_with('/') {
        return None;
    }

    let (package_name, subpath) = package_specifier_parts(&dependency.target)?;
    let package_path =
        find_node_modules_package_json(root, &dependency.source_file, &package_name)?;
    let package_text = fs::read_to_string(root.join(&package_path)).ok()?;
    let package: Value = serde_json::from_str(&package_text).ok()?;
    let package_dir = package_path.parent().unwrap_or(Path::new(""));
    resolve_package_entry(
        root,
        package_dir,
        &package,
        &subpath,
        extensions,
        package_conditions,
    )
}

fn resolve_workspace_package_exports_target(
    root: &Path,
    dependency: &Dependency,
    extensions: &[&str],
    package_conditions: &[String],
) -> Option<String> {
    if dependency.target.starts_with('.') || dependency.target.starts_with('/') {
        return None;
    }

    let (package_name, subpath) = package_specifier_parts(&dependency.target)?;
    let workspace_package_path =
        find_workspace_package_json(root, &dependency.source_file, &package_name)?;
    let package_text = fs::read_to_string(root.join(&workspace_package_path)).ok()?;
    let package: Value = serde_json::from_str(&package_text).ok()?;
    let package_dir = workspace_package_path.parent().unwrap_or(Path::new(""));
    resolve_package_entry(
        root,
        package_dir,
        &package,
        &subpath,
        extensions,
        package_conditions,
    )
}

fn find_package_json(root: &Path, source_file: &str) -> Option<PathBuf> {
    let mut current = Path::new(source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    loop {
        let candidate = current.join("package.json");
        if root.join(&candidate).is_file() {
            return Some(candidate);
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn find_workspace_package_json(
    root: &Path,
    source_file: &str,
    package_name: &str,
) -> Option<PathBuf> {
    if package_declares_catalog_dependency(root, source_file, package_name) {
        return None;
    }

    if let Some(package_path) =
        find_workspace_protocol_package_json(root, source_file, package_name)
    {
        return Some(package_path);
    }

    let mut current = Path::new(source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    loop {
        let candidate = current.join("package.json");
        if root.join(&candidate).is_file()
            && let Some(package_path) =
                find_workspace_package_from_root(root, &candidate, package_name)
        {
            return Some(package_path);
        }

        let pnpm_candidate = current.join("pnpm-workspace.yaml");
        if root.join(&pnpm_candidate).is_file()
            && let Some(package_path) =
                find_pnpm_workspace_package_from_root(root, &pnpm_candidate, package_name)
        {
            return Some(package_path);
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn find_workspace_protocol_package_json(
    root: &Path,
    source_file: &str,
    package_name: &str,
) -> Option<PathBuf> {
    let source_package_path = find_package_json(root, source_file)?;
    let source_package_text = fs::read_to_string(root.join(&source_package_path)).ok()?;
    let source_package: Value = serde_json::from_str(&source_package_text).ok()?;
    let dependency_value = workspace_protocol_dependency(&source_package, package_name)?;
    let package_relative_path = workspace_protocol_relative_path(dependency_value)?;
    let source_package_dir = source_package_path.parent().unwrap_or(Path::new(""));
    let package_path =
        normalize_path(&source_package_dir.join(package_relative_path)).join("package.json");

    root.join(&package_path).is_file().then_some(package_path)
}

fn find_workspace_package_from_root(
    root: &Path,
    workspace_package_json: &Path,
    package_name: &str,
) -> Option<PathBuf> {
    let package_text = fs::read_to_string(root.join(workspace_package_json)).ok()?;
    let package: Value = serde_json::from_str(&package_text).ok()?;
    let workspace_dir = workspace_package_json.parent().unwrap_or(Path::new(""));
    find_workspace_package_from_patterns(
        root,
        workspace_dir,
        package_workspace_patterns(&package)?,
        package_name,
    )
}

fn find_pnpm_workspace_package_from_root(
    root: &Path,
    pnpm_workspace_yaml: &Path,
    package_name: &str,
) -> Option<PathBuf> {
    let workspace_text = fs::read_to_string(root.join(pnpm_workspace_yaml)).ok()?;
    let workspace_patterns = pnpm_workspace_patterns(&workspace_text);
    if workspace_patterns.is_empty() {
        return None;
    }

    let workspace_dir = pnpm_workspace_yaml.parent().unwrap_or(Path::new(""));
    find_workspace_package_from_patterns(root, workspace_dir, workspace_patterns, package_name)
}

fn find_workspace_package_from_patterns(
    root: &Path,
    workspace_dir: &Path,
    workspace_patterns: Vec<String>,
    package_name: &str,
) -> Option<PathBuf> {
    let mut included_package_dirs = BTreeSet::new();
    let mut excluded_package_dirs = BTreeSet::new();

    for pattern in workspace_patterns {
        let (is_exclusion, pattern) = workspace_pattern_exclusion(&pattern);
        let package_dirs = expand_workspace_pattern(root, workspace_dir, pattern);
        if is_exclusion {
            excluded_package_dirs.extend(package_dirs);
        } else {
            included_package_dirs.extend(package_dirs);
        }
    }

    for package_dir in included_package_dirs {
        if workspace_package_dir_is_excluded(&package_dir, &excluded_package_dirs) {
            continue;
        }

        let package_path = package_dir.join("package.json");
        let Ok(package_text) = fs::read_to_string(root.join(&package_path)) else {
            continue;
        };
        let Ok(package) = serde_json::from_str::<Value>(&package_text) else {
            continue;
        };
        if package.get("name").and_then(Value::as_str) == Some(package_name) {
            return Some(package_path);
        }
    }

    None
}

fn workspace_pattern_exclusion(pattern: &str) -> (bool, &str) {
    pattern
        .strip_prefix('!')
        .map(|pattern| (true, pattern))
        .unwrap_or((false, pattern))
}

fn workspace_package_dir_is_excluded(
    package_dir: &Path,
    excluded_package_dirs: &BTreeSet<PathBuf>,
) -> bool {
    excluded_package_dirs
        .iter()
        .any(|excluded_dir| package_dir == excluded_dir || package_dir.starts_with(excluded_dir))
}

fn workspace_protocol_dependency<'a>(package: &'a Value, package_name: &str) -> Option<&'a str> {
    package_dependency_value(package, package_name).filter(|value| value.starts_with("workspace:"))
}

fn package_declares_catalog_dependency(root: &Path, source_file: &str, package_name: &str) -> bool {
    let Some(source_package_path) = find_package_json(root, source_file) else {
        return false;
    };
    let Ok(source_package_text) = fs::read_to_string(root.join(&source_package_path)) else {
        return false;
    };
    let Ok(source_package) = serde_json::from_str::<Value>(&source_package_text) else {
        return false;
    };
    let Some(catalog_name) =
        package_dependency_value(&source_package, package_name).and_then(catalog_protocol_name)
    else {
        return false;
    };

    pnpm_workspace_declares_catalog_dependency(
        root,
        &source_package_path,
        catalog_name,
        package_name,
    )
    .unwrap_or(true)
}

fn package_dependency_value<'a>(package: &'a Value, package_name: &str) -> Option<&'a str> {
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(value) = package
            .get(section)
            .and_then(Value::as_object)
            .and_then(|dependencies| dependencies.get(package_name))
            .and_then(Value::as_str)
        {
            return Some(value);
        }
    }
    None
}

fn catalog_protocol_name(value: &str) -> Option<&str> {
    let catalog_name = value.strip_prefix("catalog:")?.trim();
    Some(if catalog_name.is_empty() {
        "default"
    } else {
        catalog_name
    })
}

fn workspace_protocol_relative_path(value: &str) -> Option<PathBuf> {
    let target = value.strip_prefix("workspace:")?;
    (target.starts_with("./") || target.starts_with("../")).then(|| PathBuf::from(target))
}

fn package_workspace_patterns(package: &Value) -> Option<Vec<String>> {
    let workspaces = package.get("workspaces")?;
    if let Some(patterns) = workspaces.as_array() {
        return Some(
            patterns
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        );
    }

    workspaces
        .get("packages")
        .and_then(Value::as_array)
        .map(|patterns| {
            patterns
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
}

fn pnpm_workspace_patterns(text: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut in_packages = false;

    for raw_line in text.lines() {
        let line = strip_yaml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if !raw_line.starts_with([' ', '\t']) {
            in_packages = false;
            if let Some(rest) = line.strip_prefix("packages:") {
                in_packages = true;
                patterns.extend(yaml_string_values(rest.trim()));
            }
            continue;
        }

        if in_packages && let Some(rest) = line.strip_prefix('-') {
            patterns.extend(yaml_string_values(rest.trim()));
        }
    }

    patterns
}

fn pnpm_workspace_declares_catalog_dependency(
    root: &Path,
    source_package_path: &Path,
    catalog_name: &str,
    package_name: &str,
) -> Option<bool> {
    let source_package_dir = source_package_path.parent().unwrap_or(Path::new(""));
    let workspace_path = find_nearest_pnpm_workspace_yaml(root, source_package_dir)?;
    let workspace_text = fs::read_to_string(root.join(workspace_path)).ok()?;
    Some(pnpm_workspace_catalog_declares_dependency(
        &workspace_text,
        catalog_name,
        package_name,
    ))
}

fn find_nearest_pnpm_workspace_yaml(root: &Path, from_dir: &Path) -> Option<PathBuf> {
    let mut current = from_dir.to_path_buf();

    loop {
        let candidate = current.join("pnpm-workspace.yaml");
        if root.join(&candidate).is_file() {
            return Some(candidate);
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn pnpm_workspace_catalog_declares_dependency(
    text: &str,
    catalog_name: &str,
    package_name: &str,
) -> bool {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CatalogSection {
        Default,
        Named,
    }

    let mut section = None;
    let mut current_named_catalog = None::<String>;
    let mut current_named_catalog_indent = 0;

    for raw_line in text.lines() {
        let line = strip_yaml_comment(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let indent = yaml_line_indent(line);
        if indent == 0 {
            section = None;
            current_named_catalog = None;
            current_named_catalog_indent = 0;

            if trimmed.starts_with("catalog:") {
                section = Some(CatalogSection::Default);
                continue;
            }
            if trimmed.starts_with("catalogs:") {
                section = Some(CatalogSection::Named);
                continue;
            }
        }

        match section {
            Some(CatalogSection::Default) if catalog_name == "default" => {
                if yaml_mapping_key(trimmed).as_deref() == Some(package_name) {
                    return true;
                }
            }
            Some(CatalogSection::Named) => {
                if current_named_catalog.is_none() || indent <= current_named_catalog_indent {
                    current_named_catalog = yaml_mapping_key(trimmed);
                    current_named_catalog_indent = indent;
                    continue;
                }

                if current_named_catalog.as_deref() == Some(catalog_name)
                    && yaml_mapping_key(trimmed).as_deref() == Some(package_name)
                {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}

fn yaml_line_indent(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .count()
}

fn yaml_mapping_key(line: &str) -> Option<String> {
    let (key, _) = line.split_once(':')?;
    clean_yaml_string(key)
}

fn yaml_string_values(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }

    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return inner
            .split(',')
            .filter_map(clean_yaml_string)
            .collect::<Vec<_>>();
    }

    clean_yaml_string(value).into_iter().collect()
}

fn clean_yaml_string(value: &str) -> Option<String> {
    let value = value.trim().trim_matches(['"', '\'']);
    (!value.is_empty()).then(|| value.to_string())
}

fn strip_yaml_comment(line: &str) -> &str {
    line.split_once('#')
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn expand_workspace_pattern(root: &Path, workspace_dir: &Path, pattern: &str) -> Vec<PathBuf> {
    if Path::new(pattern).is_absolute() {
        return Vec::new();
    }

    let segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    expand_workspace_pattern_segments(root, workspace_dir.to_path_buf(), &segments, &mut matches);
    matches
}

fn expand_workspace_pattern_segments(
    root: &Path,
    base: PathBuf,
    segments: &[&str],
    matches: &mut Vec<PathBuf>,
) {
    let Some((segment, remaining)) = segments.split_first() else {
        if root.join(&base).join("package.json").is_file() {
            matches.push(normalize_path(&base));
        }
        return;
    };

    if *segment == "**" {
        expand_workspace_pattern_segments(root, base.clone(), remaining, matches);
        for child_dir in workspace_child_dirs(root, &base) {
            expand_workspace_pattern_segments(root, base.join(child_dir), segments, matches);
        }
        return;
    }

    if segment.contains('*') {
        for child_dir in workspace_child_dirs(root, &base) {
            let child_name = child_dir.to_string_lossy();
            if workspace_segment_matches(segment, &child_name) {
                expand_workspace_pattern_segments(root, base.join(child_dir), remaining, matches);
            }
        }
    } else {
        expand_workspace_pattern_segments(root, base.join(segment), remaining, matches);
    }
}

fn workspace_child_dirs(root: &Path, base: &Path) -> Vec<std::ffi::OsString> {
    let Ok(entries) = fs::read_dir(root.join(base)) else {
        return Vec::new();
    };
    let mut child_dirs = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_dir())
                .map(|_| entry.file_name())
        })
        .collect::<Vec<_>>();
    child_dirs.sort();
    child_dirs
}

fn workspace_segment_matches(pattern: &str, value: &str) -> bool {
    let Some(star_index) = pattern.find('*') else {
        return pattern == value;
    };
    if pattern[star_index + 1..].contains('*') {
        return false;
    }

    let prefix = &pattern[..star_index];
    let suffix = &pattern[star_index + 1..];
    value.starts_with(prefix)
        && value.ends_with(suffix)
        && value.len() >= prefix.len() + suffix.len()
}

fn find_node_modules_package_json(
    root: &Path,
    source_file: &str,
    package_name: &str,
) -> Option<PathBuf> {
    let mut current = Path::new(source_file)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();

    loop {
        let candidate = current
            .join("node_modules")
            .join(package_name)
            .join("package.json");
        if root.join(&candidate).is_file() {
            return Some(candidate);
        }

        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn package_specifier_parts(target: &str) -> Option<(String, String)> {
    if target.starts_with('@') {
        let mut parts = target.splitn(3, '/');
        let scope = parts.next()?;
        let name = parts.next()?;
        if scope.len() <= 1 || name.is_empty() {
            return None;
        }
        let package_name = format!("{scope}/{name}");
        let subpath = parts
            .next()
            .map(|rest| format!("./{rest}"))
            .unwrap_or_else(|| ".".to_string());
        return Some((package_name, subpath));
    }

    let mut parts = target.splitn(2, '/');
    let package_name = parts.next()?;
    if package_name.is_empty() {
        return None;
    }
    let subpath = parts
        .next()
        .map(|rest| format!("./{rest}"))
        .unwrap_or_else(|| ".".to_string());
    Some((package_name.to_string(), subpath))
}

fn package_export_subpath(package_name: &str, target: &str) -> Option<String> {
    if target == package_name {
        return Some(".".to_string());
    }
    target
        .strip_prefix(package_name)
        .and_then(|rest| rest.strip_prefix('/'))
        .map(|rest| format!("./{rest}"))
}

fn resolve_package_entry(
    root: &Path,
    package_dir: &Path,
    package: &Value,
    subpath: &str,
    extensions: &[&str],
    package_conditions: &[String],
) -> Option<String> {
    let mappings = package_entry_mappings(package, subpath, package_conditions);

    for mapped in package_browser_mappings(package, subpath, &mappings) {
        if let Some(resolved) = resolve_base(root, package_dir.join(mapped), extensions) {
            return Some(resolved);
        }
    }
    None
}

fn package_entry_mappings(
    package: &Value,
    subpath: &str,
    package_conditions: &[String],
) -> Vec<PathBuf> {
    if let Some(exports) = package.get("exports")
        && let Some(mappings) = package_export_mappings(exports, subpath, package_conditions)
    {
        return mappings;
    }

    package_metadata_entry(package, subpath)
        .into_iter()
        .collect()
}

fn package_browser_mappings(package: &Value, subpath: &str, mappings: &[PathBuf]) -> Vec<PathBuf> {
    let Some(browser) = package.get("browser") else {
        return mappings.to_vec();
    };

    if subpath == "."
        && let Some(browser_entry) = browser.as_str()
    {
        return package_local_target_path(browser_entry.to_string())
            .into_iter()
            .collect();
    }

    let Some(browser_entries) = browser.as_object() else {
        return mappings.to_vec();
    };

    mappings
        .iter()
        .filter_map(|mapping| package_browser_mapping(browser_entries, mapping))
        .collect()
}

fn package_browser_mapping(
    browser_entries: &serde_json::Map<String, Value>,
    mapping: &Path,
) -> Option<PathBuf> {
    let mapping = mapping.to_string_lossy().replace('\\', "/");
    let with_dot = if mapping.starts_with("./") {
        mapping.clone()
    } else {
        format!("./{mapping}")
    };
    let without_dot = mapping.strip_prefix("./").unwrap_or(&mapping);

    for key in [mapping.as_str(), with_dot.as_str(), without_dot] {
        if let Some(value) = browser_entries.get(key) {
            if value.as_bool() == Some(false) {
                return None;
            }
            return value
                .as_str()
                .and_then(|target| package_local_target_path(target.to_string()));
        }
    }

    Some(PathBuf::from(mapping))
}

fn package_export_mappings(
    exports: &Value,
    subpath: &str,
    package_conditions: &[String],
) -> Option<Vec<PathBuf>> {
    if subpath == "." && !package_exports_uses_subpath_keys(exports) {
        return Some(
            package_export_targets(exports, package_conditions)
                .unwrap_or_default()
                .into_iter()
                .filter_map(package_local_target_path)
                .collect(),
        );
    }

    let entries = exports.as_object()?;
    let (value, wildcards) = path_pattern_mapping(entries, subpath)?;
    Some(
        package_export_targets(value, package_conditions)
            .unwrap_or_default()
            .into_iter()
            .filter_map(package_local_target_path)
            .filter_map(|target| apply_path_mapping(&target.to_string_lossy(), &wildcards))
            .collect(),
    )
}

fn package_exports_uses_subpath_keys(exports: &Value) -> bool {
    exports
        .as_object()
        .map(|entries| entries.keys().any(|key| key.starts_with('.')))
        .unwrap_or(false)
}

fn package_import_mappings(
    imports: &Value,
    target: &str,
    package_conditions: &[String],
) -> Option<Vec<PathBuf>> {
    let entries = imports.as_object()?;
    let (value, wildcards) = path_pattern_mapping(entries, target)?;
    Some(
        package_export_targets(value, package_conditions)
            .unwrap_or_default()
            .into_iter()
            .filter_map(package_local_target_path)
            .filter_map(|target| apply_path_mapping(&target.to_string_lossy(), &wildcards))
            .collect(),
    )
}

fn path_pattern_mapping<'a>(
    entries: &'a serde_json::Map<String, Value>,
    target: &str,
) -> Option<(&'a Value, Vec<String>)> {
    if let Some(value) = entries.get(target) {
        return Some((value, Vec::new()));
    }

    entries
        .iter()
        .filter(|(pattern, _)| pattern.contains('*'))
        .filter_map(|(pattern, value)| {
            let wildcards = path_pattern_captures(pattern, target)?;
            Some((pattern, value, wildcards))
        })
        .max_by(|(left, _, _), (right, _, _)| {
            path_pattern_specificity(left).cmp(&path_pattern_specificity(right))
        })
        .map(|(_, value, wildcards)| (value, wildcards))
}

fn path_pattern_specificity(pattern: &str) -> (usize, usize) {
    (pattern.find('*').unwrap_or(pattern.len()), pattern.len())
}

fn package_local_target_path(target: String) -> Option<PathBuf> {
    (target.starts_with("./") || target.starts_with("../")).then(|| PathBuf::from(target))
}

fn package_metadata_entry(package: &Value, subpath: &str) -> Option<PathBuf> {
    if subpath == "." {
        for field in ["module", "main", "types", "typings"] {
            if let Some(target) = package.get(field).and_then(Value::as_str)
                && let Some(target) = package_local_target_path(target.to_string())
            {
                return Some(target);
            }
        }
        return None;
    }

    subpath.strip_prefix("./").map(PathBuf::from)
}

fn package_export_targets(value: &Value, package_conditions: &[String]) -> Option<Vec<String>> {
    if let Some(target) = value.as_str() {
        return Some(vec![target.to_string()]);
    }

    if let Some(targets) = value.as_array() {
        return Some(
            targets
                .iter()
                .flat_map(|target| {
                    package_export_targets(target, package_conditions).unwrap_or_default()
                })
                .collect(),
        );
    }

    if value.is_null() {
        return Some(Vec::new());
    }

    let Some(object) = value.as_object() else {
        return Some(Vec::new());
    };
    for condition in package_conditions {
        if let Some(target) = object.get(condition) {
            return package_export_targets(target, package_conditions);
        }
    }
    None
}

fn resolve_relative_target(
    root: &Path,
    source_file: &str,
    target: &str,
    extensions: &[&str],
) -> Option<String> {
    if !(target.starts_with("./") || target.starts_with("../") || target.starts_with('/')) {
        return None;
    }

    let source_dir = Path::new(source_file).parent().unwrap_or(Path::new(""));
    let base = if let Some(stripped) = target.strip_prefix('/') {
        PathBuf::from(stripped)
    } else {
        source_dir.join(target)
    };

    resolve_base(root, base, extensions)
}

fn resolve_base(root: &Path, base: PathBuf, extensions: &[&str]) -> Option<String> {
    let mut candidates = vec![base.clone()];
    for extension in extensions {
        candidates.push(base.with_extension(extension));
        candidates.push(base.join(format!("index.{extension}")));
        candidates.push(base.join(format!("__init__.{extension}")));
    }
    existing_relative(root, candidates)
}

fn existing_relative<I>(root: &Path, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = PathBuf>,
{
    for candidate in candidates {
        let full = root.join(&candidate);
        if full.is_file() {
            let normalized = normalize_path(&candidate);
            return Some(normalized.to_string_lossy().replace('\\', "/"));
        }
    }
    None
}

fn string_literal_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut in_quote = false;
    let mut quote = '\0';
    let mut current = String::new();

    for ch in text.chars() {
        if in_quote {
            if ch == quote {
                if !current.is_empty() {
                    targets.push(current.clone());
                }
                current.clear();
                in_quote = false;
            } else {
                current.push(ch);
            }
        } else if matches!(ch, '"' | '\'' | '`') {
            in_quote = true;
            quote = ch;
        }
    }

    targets
}

fn go_import_specs(text: &str) -> Vec<(String, Option<String>)> {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("import") else {
        return Vec::new();
    };
    let rest = rest.trim();
    let specs = if rest.starts_with('(') && rest.ends_with(')') {
        rest.trim_start_matches('(')
            .trim_end_matches(')')
            .lines()
            .collect::<Vec<_>>()
    } else {
        vec![rest]
    };

    specs.into_iter().filter_map(go_import_spec).collect()
}

fn go_import_spec(spec: &str) -> Option<(String, Option<String>)> {
    let spec = spec
        .split("//")
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(';')
        .trim();
    if spec.is_empty() {
        return None;
    }

    let quote_index = spec
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '"' | '\'' | '`').then_some(index))?;
    let target = string_literal_targets(&spec[quote_index..])
        .into_iter()
        .next()?;
    let alias = spec[..quote_index].split_whitespace().last();
    let local_alias = match alias {
        Some("_" | ".") => None,
        Some(alias) => Some(alias.to_string()),
        None => target
            .rsplit('/')
            .find(|part| !part.is_empty())
            .map(str::to_string),
    };

    Some((target, local_alias))
}

fn java_import_alias(target: &str, kind: &str) -> (Option<String>, Option<String>) {
    let mut parts = target
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let Some(local_alias) = parts.pop() else {
        return (None, None);
    };

    match kind {
        "static_import" => (Some(local_alias.to_string()), Some(local_alias.to_string())),
        "import" => (Some(local_alias.to_string()), Some("*".to_string())),
        _ => (None, None),
    }
}

#[cfg(test)]
fn python_import_targets(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("from ") {
        let Some((module, imports)) = rest.split_once(" import ") else {
            return rest
                .split_whitespace()
                .next()
                .map(|target| vec![target.to_string()])
                .unwrap_or_default();
        };
        let module = module.trim();
        if module.is_empty() {
            return Vec::new();
        }

        let mut targets = vec![module.to_string()];
        for (imported, _) in python_from_import_aliases(imports) {
            targets.push(python_join_import_target(module, &imported));
        }
        targets.sort();
        targets.dedup();
        return targets;
    }

    trimmed
        .strip_prefix("import ")
        .map(|rest| {
            python_import_aliases(rest)
                .into_iter()
                .map(|(target, _)| target)
                .collect()
        })
        .unwrap_or_default()
}

fn python_from_import_aliases(text: &str) -> Vec<(String, String)> {
    text.trim()
        .trim_matches(['(', ')'])
        .split(',')
        .filter_map(python_alias_pair)
        .filter(|(imported, _)| imported != "*")
        .collect()
}

fn python_import_aliases(text: &str) -> Vec<(String, Option<String>)> {
    text.split(',')
        .filter_map(python_alias_pair)
        .map(|(target, alias)| {
            let local_alias = if alias == target { None } else { Some(alias) };
            (target, local_alias)
        })
        .collect()
}

fn python_alias_pair(part: &str) -> Option<(String, String)> {
    let mut tokens = part.split_whitespace();
    let imported = tokens.next()?.trim();
    if imported.is_empty() {
        return None;
    }
    let alias = match (tokens.next(), tokens.next()) {
        (Some("as"), Some(alias)) => alias.trim(),
        _ => imported,
    };
    if alias.is_empty() {
        return None;
    }

    Some((imported.to_string(), alias.to_string()))
}

fn python_join_import_target(module: &str, imported: &str) -> String {
    if module.chars().all(|character| character == '.') {
        format!("{module}{imported}")
    } else {
        format!("{module}.{imported}")
    }
}

fn java_import_target(text: &str) -> Option<(String, &'static str)> {
    let target = text.trim().strip_prefix("import ")?.trim();
    let (target, kind) = target
        .strip_prefix("static ")
        .map(|target| (target.trim(), "import_static"))
        .unwrap_or((target, "import"));
    let target = target.trim_end_matches(';').trim();
    (!target.is_empty()).then(|| (target.to_string(), kind))
}

fn java_package_targets(text: &str) -> Vec<String> {
    text.trim()
        .strip_prefix("package ")
        .map(|target| target.trim_end_matches(';').trim())
        .filter(|target| !target.is_empty())
        .map(|target| vec![target.to_string()])
        .unwrap_or_default()
}

fn c_include_targets(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('"')
        && let Some(end) = trimmed[start + 1..].find('"')
    {
        return vec![trimmed[start + 1..start + 1 + end].to_string()];
    }

    if let Some(start) = trimmed.find('<')
        && let Some(end) = trimmed[start + 1..].find('>')
    {
        return vec![format!("<{}>", &trimmed[start + 1..start + 1 + end])];
    }

    Vec::new()
}

fn csharp_using_target(text: &str) -> Option<(String, &'static str)> {
    let trimmed = text.trim();
    if trimmed.starts_with("using static ") {
        let target = trimmed
            .strip_prefix("using static ")?
            .trim_end_matches(';')
            .trim();
        return (!target.is_empty()).then(|| (target.to_string(), "using_static"));
    }

    let target = trimmed.strip_prefix("using ")?;
    let is_alias = target.contains('=');
    let target = target
        .split('=')
        .next_back()
        .unwrap_or(target)
        .trim_end_matches(';')
        .trim();
    (!target.is_empty() && !target.starts_with('(')).then(|| {
        (
            target.to_string(),
            if is_alias { "using_alias" } else { "using" },
        )
    })
}

fn csharp_namespace_targets(text: &str) -> Vec<String> {
    text.trim()
        .strip_prefix("namespace ")
        .map(|target| target.split([';', '{']).next().unwrap_or_default().trim())
        .filter(|target| !target.is_empty())
        .map(|target| vec![target.to_string()])
        .unwrap_or_default()
}

fn csharp_direct_base_type(text: &str) -> Option<String> {
    let (_, rest) = text.split_once(':')?;
    let candidate = rest
        .split(['{', ';'])
        .next()
        .unwrap_or_default()
        .split(',')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim();
    clean_csharp_type_name(candidate).map(ToOwned::to_owned)
}

fn csharp_extension_receiver_type(text: &str) -> Option<String> {
    let params_open = text.find('(')?;
    let params_close = matching_close_paren(text, params_open)?;
    let params = text[params_open + 1..params_close].trim();
    let first_param = csharp_first_parameter(params)?.trim();
    let receiver = first_param.strip_prefix("this ")?.trim_start();
    let (raw_type, _) = csharp_type_binding_parts(receiver)?;
    clean_csharp_type_name(raw_type).map(ToOwned::to_owned)
}

fn csharp_first_parameter(params: &str) -> Option<&str> {
    let mut angle_depth = 0;
    for (index, character) in params.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if angle_depth == 0 => {
                let first = params[..index].trim();
                return (!first.is_empty()).then_some(first);
            }
            _ => {}
        }
    }

    let first = params.trim();
    (!first.is_empty()).then_some(first)
}

fn csharp_type_bindings(text: &str) -> Vec<(String, String, Option<String>)> {
    let mut text = text.trim().trim_end_matches(';').trim();
    while let Some((modifier, rest)) = text.split_once(char::is_whitespace) {
        if !csharp_type_binding_modifier(modifier) {
            break;
        }
        text = rest.trim_start();
    }

    let Some((raw_type, rest)) = csharp_type_binding_parts(text) else {
        return Vec::new();
    };
    let local_alias = csharp_local_alias(rest).unwrap_or_default();

    if local_alias.is_empty() || !is_js_identifier(local_alias) {
        return Vec::new();
    }

    let (target, wrapper_member) = if raw_type.trim() == "var" {
        let Some((target, wrapper_member)) = csharp_new_expression_binding(rest) else {
            return Vec::new();
        };
        (Some(target), wrapper_member)
    } else if let Some((target, wrapper_member)) = csharp_value_wrapper_type(raw_type) {
        (
            clean_csharp_type_name(target).map(ToOwned::to_owned),
            Some(wrapper_member.to_string()),
        )
    } else {
        (
            clean_csharp_type_name(raw_type).map(ToOwned::to_owned),
            None,
        )
    };

    target
        .map(|target| vec![(target, local_alias.to_string(), wrapper_member)])
        .unwrap_or_default()
}

fn csharp_local_alias(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    let end = rest
        .char_indices()
        .find_map(|(index, character)| {
            (!(character == '_' || character == '$' || character.is_ascii_alphanumeric()))
                .then_some(index)
        })
        .unwrap_or(rest.len());
    let alias = rest[..end].trim();
    (!alias.is_empty()).then_some(alias)
}

fn csharp_type_binding_parts(text: &str) -> Option<(&str, &str)> {
    let mut angle_depth = 0;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            _ if character.is_whitespace() && angle_depth == 0 => {
                let raw_type = text[..index].trim();
                let rest = text[index..].trim_start();
                if raw_type.is_empty() || rest.is_empty() {
                    return None;
                }
                return Some((raw_type, rest));
            }
            _ => {}
        }
    }

    None
}

fn csharp_type_binding_modifier(value: &str) -> bool {
    matches!(
        value,
        "private"
            | "protected"
            | "internal"
            | "public"
            | "static"
            | "readonly"
            | "const"
            | "volatile"
            | "ref"
            | "out"
            | "in"
            | "params"
    )
}

fn clean_csharp_type_name(value: &str) -> Option<&str> {
    let value = value
        .trim()
        .strip_prefix("global::")
        .unwrap_or(value)
        .trim_end_matches('?')
        .trim_end_matches("[]");
    if let Some(element_type) = csharp_collection_element_type(value) {
        return clean_csharp_type_name(element_type);
    }
    if let Some(value_type) = csharp_dictionary_value_type(value) {
        return clean_csharp_type_name(value_type);
    }
    if value.is_empty()
        || value.contains('<')
        || value.contains('>')
        || csharp_builtin_type(value)
        || !value
            .split('.')
            .all(|segment| !segment.is_empty() && is_js_identifier(segment))
    {
        return None;
    }

    Some(value)
}

fn csharp_collection_element_type(value: &str) -> Option<&str> {
    let (collection_type, arguments) = csharp_generic_type_arguments(value)?;
    if arguments.len() != 1 {
        return None;
    }
    let element_type = arguments[0];
    if element_type.contains('<') || element_type.contains('>') {
        return None;
    }

    let collection_name = collection_type
        .trim()
        .split('.')
        .next_back()
        .unwrap_or(collection_type)
        .trim();
    csharp_collection_type(collection_name).then_some(element_type)
}

fn csharp_dictionary_value_type(value: &str) -> Option<&str> {
    let (dictionary_type, arguments) = csharp_generic_type_arguments(value)?;
    if arguments.len() != 2 {
        return None;
    }
    let value_type = arguments[1];
    if value_type.contains('<') || value_type.contains('>') {
        return None;
    }

    let dictionary_name = dictionary_type
        .trim()
        .split('.')
        .next_back()
        .unwrap_or(dictionary_type)
        .trim();
    csharp_dictionary_type(dictionary_name).then_some(value_type)
}

fn csharp_value_wrapper_type(value: &str) -> Option<(&str, &str)> {
    let value = value
        .trim()
        .strip_prefix("global::")
        .unwrap_or(value)
        .trim_end_matches('?');
    let (wrapper_type, arguments) = csharp_generic_type_arguments(value)?;
    if arguments.len() != 1 {
        return None;
    }
    let inner_type = arguments[0];
    if inner_type.contains('<') || inner_type.contains('>') {
        return None;
    }

    let wrapper_name = wrapper_type
        .trim()
        .split('.')
        .next_back()
        .unwrap_or(wrapper_type)
        .trim();
    csharp_value_wrapper_member(wrapper_name).map(|member| (inner_type, member))
}

fn csharp_generic_type_arguments(value: &str) -> Option<(&str, Vec<&str>)> {
    let (raw_type, raw_arguments) = value.split_once('<')?;
    let raw_arguments = raw_arguments.strip_suffix('>')?;
    let mut arguments = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0;

    for (index, character) in raw_arguments.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if angle_depth == 0 => {
                let argument = raw_arguments[start..index].trim();
                if argument.is_empty() {
                    return None;
                }
                arguments.push(argument);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    let argument = raw_arguments[start..].trim();
    if argument.is_empty() {
        return None;
    }
    arguments.push(argument);
    Some((raw_type, arguments))
}

fn csharp_collection_type(value: &str) -> bool {
    matches!(
        value,
        "List"
            | "IList"
            | "IEnumerable"
            | "ICollection"
            | "IReadOnlyList"
            | "IReadOnlyCollection"
            | "HashSet"
    )
}

fn csharp_dictionary_type(value: &str) -> bool {
    matches!(value, "Dictionary" | "IDictionary" | "IReadOnlyDictionary")
}

fn csharp_value_wrapper_member(value: &str) -> Option<&'static str> {
    match value {
        "Lazy" => Some("Value"),
        "Task" | "ValueTask" => Some("Result"),
        _ => None,
    }
}

fn csharp_new_expression_binding(value: &str) -> Option<(String, Option<String>)> {
    let initializer = value.split_once('=')?.1.trim();
    let constructor = initializer.strip_prefix("new ")?.trim_start();
    let target = constructor
        .split(['(', '{', '[', ';'])
        .next()
        .unwrap_or_default()
        .trim();
    if let Some((target, wrapper_member)) = csharp_value_wrapper_type(target) {
        return clean_csharp_type_name(target)
            .map(|target| (target.to_string(), Some(wrapper_member.to_string())));
    }

    clean_csharp_type_name(target).map(|target| (target.to_string(), None))
}

fn csharp_builtin_type(value: &str) -> bool {
    matches!(
        value,
        "bool"
            | "byte"
            | "sbyte"
            | "char"
            | "decimal"
            | "double"
            | "float"
            | "int"
            | "uint"
            | "long"
            | "ulong"
            | "object"
            | "short"
            | "ushort"
            | "string"
            | "var"
            | "void"
            | "dynamic"
    )
}

fn csharp_using_alias(text: &str, target: &str, kind: &str) -> (Option<String>, Option<String>) {
    match kind {
        "using_alias" => {
            let alias = text
                .trim()
                .strip_prefix("using ")
                .and_then(|rest| rest.split_once('='))
                .map(|(alias, _)| alias.trim())
                .filter(|alias| !alias.is_empty());
            match alias {
                Some(alias) => (Some(alias.to_string()), Some("*".to_string())),
                None => (None, None),
            }
        }
        "using_static" => (None, Some("*".to_string())),
        "using" => target
            .split('.')
            .next_back()
            .filter(|alias| !alias.is_empty())
            .map(|alias| (Some(alias.to_string()), Some("*".to_string())))
            .unwrap_or((None, None)),
        _ => (None, None),
    }
}

fn php_use_entries(text: &str) -> Vec<(String, Option<String>, Option<String>)> {
    text.trim()
        .strip_prefix("use ")
        .map(php_use_entries_from_target)
        .unwrap_or_default()
}

fn php_use_entries_from_target(target: &str) -> Vec<(String, Option<String>, Option<String>)> {
    let target = target.trim().trim_end_matches(';').trim();
    let (target, use_kind) = if let Some(target) = target.strip_prefix("function ") {
        (target.trim(), "function")
    } else if let Some(target) = target.strip_prefix("const ") {
        (target.trim(), "const")
    } else {
        (target, "class")
    };

    if target.contains('{') {
        return php_grouped_use_entries(target, use_kind);
    }

    target
        .split(',')
        .filter_map(|part| php_use_entry(part, use_kind))
        .collect()
}

fn php_grouped_use_entries(
    target: &str,
    use_kind: &str,
) -> Vec<(String, Option<String>, Option<String>)> {
    let Some(open) = target.find('{') else {
        return Vec::new();
    };
    let Some(close) = matching_delimiter(target, open, '{', '}') else {
        return Vec::new();
    };
    let prefix = target[..open]
        .trim()
        .trim_start_matches('\\')
        .trim_end_matches('\\')
        .trim();
    let members = &target[open + 1..close];

    split_top_level_commas(members)
        .into_iter()
        .filter_map(|member| {
            let member = member.trim();
            if member.is_empty() {
                return None;
            }

            let (member, member_kind) = if let Some(member) = member.strip_prefix("function ") {
                (member.trim(), "function")
            } else if let Some(member) = member.strip_prefix("const ") {
                (member.trim(), "const")
            } else {
                (member, use_kind)
            };

            let full_target = if prefix.is_empty() {
                member.to_string()
            } else {
                format!("{prefix}\\{member}")
            };
            php_use_entry(&full_target, member_kind)
        })
        .collect()
}

fn php_use_entry(part: &str, use_kind: &str) -> Option<(String, Option<String>, Option<String>)> {
    let part = part.trim();
    if part.is_empty() || part.contains('{') {
        return None;
    }

    let (target, explicit_alias) = split_case_insensitive_once(part, " as ")
        .map(|(target, alias)| (target.trim(), Some(alias.trim())))
        .unwrap_or((part, None));
    let target = target.trim_start_matches('\\').trim();
    if target.is_empty() {
        return None;
    }

    let imported = target
        .rsplit('\\')
        .find(|segment| !segment.trim().is_empty())?
        .trim()
        .to_string();
    let local_alias = explicit_alias
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .or_else(|| Some(imported.clone()));
    let imported_symbol = match use_kind {
        "function" | "const" => Some(imported),
        _ => Some("*".to_string()),
    };

    Some((target.to_string(), local_alias, imported_symbol))
}

fn split_case_insensitive_once<'a>(text: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let text_lower = text.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let index = text_lower.find(&needle_lower)?;
    Some((&text[..index], &text[index + needle.len()..]))
}

#[cfg(test)]
fn rust_use_targets(text: &str) -> Vec<String> {
    rust_use_entries(text)
        .into_iter()
        .map(|(target, _)| target)
        .collect()
}

fn rust_use_entries(text: &str) -> Vec<(String, Option<String>)> {
    text.trim()
        .strip_prefix("use ")
        .map(|target| {
            let cleaned = target
                .trim_end_matches(';')
                .trim()
                .replace("::{", "::")
                .replace(['{', '}'], "");
            let cleaned = compact_whitespace(&cleaned);
            let (target, alias) = if let Some((target, alias)) = cleaned.rsplit_once(" as ") {
                (target.trim(), Some(alias.trim().to_string()))
            } else {
                (cleaned.trim(), None)
            };
            vec![(target.to_string(), alias.filter(|alias| !alias.is_empty()))]
        })
        .unwrap_or_default()
}

fn rust_use_alias(target: &str, explicit_alias: Option<&str>) -> (Option<String>, Option<String>) {
    let trimmed = target.trim();
    if trimmed.is_empty() || trimmed.contains(',') {
        return (None, None);
    }

    let imported_symbol = trimmed
        .rsplit("::")
        .find(|part| {
            let part = part.trim();
            !part.is_empty() && !matches!(part, "crate" | "self" | "super")
        })
        .map(str::to_string);
    let local_alias = explicit_alias
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .or_else(|| imported_symbol.clone());

    match (local_alias, imported_symbol) {
        (Some(local_alias), Some(imported_symbol)) => {
            if local_alias == imported_symbol {
                (Some(local_alias), Some("*".to_string()))
            } else {
                (Some(local_alias), Some(imported_symbol))
            }
        }
        _ => (None, None),
    }
}

fn rust_mod_targets(text: &str) -> Vec<String> {
    text.trim()
        .strip_prefix("mod ")
        .and_then(|target| {
            target
                .split(|ch: char| ch.is_whitespace() || matches!(ch, ';' | '{'))
                .find(|part| !part.is_empty())
        })
        .map(|target| vec![target.to_string()])
        .unwrap_or_default()
}

fn compact_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn visit_children(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    file: &str,
    scope: &mut Vec<String>,
    symbols: &mut Vec<Symbol>,
) {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        visit_node(child, source, language, file, scope, symbols);
    }
}

fn node_children<'tree>(cursor: &mut TreeCursor<'tree>) -> Vec<Node<'tree>> {
    let mut children = Vec::new();
    if cursor.goto_first_child() {
        loop {
            children.push(cursor.node());
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    children
}

fn symbol_from_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
) -> Option<(String, SymbolKind)> {
    match language {
        Language::Bash => bash_symbol(node, source),
        Language::C | Language::Cpp => c_like_symbol(node, source),
        Language::CSharp => csharp_symbol(node, source),
        Language::Php => php_symbol(node, source),
        Language::Python => python_symbol(node, source),
        Language::Ruby => ruby_symbol(node, source),
        Language::Rust => rust_symbol(node, source),
        Language::Go => go_symbol(node, source),
        Language::Java => java_symbol(node, source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_like_symbol(node, source)
        }
    }
}

fn bash_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "function_definition" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Function))
        }
        _ => None,
    }
}

fn python_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "function_definition" => child_text(node, "name", source).map(|name| {
            let kind = if is_inside_class(node) {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            (name, kind)
        }),
        "class_definition" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        _ => None,
    }
}

fn rust_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "function_item" => child_text(node, "name", source).map(|name| {
            let kind = if is_inside_rust_impl(node) {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            (name, kind)
        }),
        "struct_item" => child_text(node, "name", source).map(|name| (name, SymbolKind::Struct)),
        "enum_item" => child_text(node, "name", source).map(|name| (name, SymbolKind::Struct)),
        "trait_item" => child_text(node, "name", source).map(|name| (name, SymbolKind::Interface)),
        "const_item" => child_text(node, "name", source).map(|name| (name, SymbolKind::Constant)),
        "static_item" => child_text(node, "name", source).map(|name| (name, SymbolKind::Variable)),
        _ => None,
    }
}

fn go_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "function_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Function))
        }
        "method_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "type_declaration" => find_go_type_name(node, source),
        "const_declaration" => find_go_value_name(node, source, SymbolKind::Constant),
        "var_declaration" => find_go_value_name(node, source, SymbolKind::Variable),
        _ => None,
    }
}

fn c_like_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "function_definition" => {
            find_c_function_name(node, source).map(|name| (name, SymbolKind::Function))
        }
        "declaration" => {
            find_c_function_declaration_name(node, source).map(|name| (name, SymbolKind::Function))
        }
        "struct_specifier" | "union_specifier" | "class_specifier" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Struct))
        }
        "enum_specifier" => child_text(node, "name", source).map(|name| (name, SymbolKind::Struct)),
        "type_definition" => find_c_typedef_name(node, source),
        "preproc_function_def" | "preproc_def" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Constant))
        }
        _ => None,
    }
}

fn csharp_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "class_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        "interface_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Interface))
        }
        "struct_declaration" | "enum_declaration" | "record_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Struct))
        }
        "method_declaration" | "constructor_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "property_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "field_declaration" => find_csharp_field_name(node, source),
        _ => None,
    }
}

fn php_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "class_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        "interface_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Interface))
        }
        "trait_declaration" | "enum_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Struct))
        }
        "function_definition" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Function))
        }
        "method_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "property_declaration" => find_php_property_name(node, source),
        "const_declaration" => find_php_const_name(node, source),
        _ => None,
    }
}

fn ruby_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "class" => child_text(node, "name", source).map(|name| (name, SymbolKind::Class)),
        "module" => child_text(node, "name", source).map(|name| (name, SymbolKind::Interface)),
        "method" | "singleton_method" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "assignment" => find_ruby_constant_assignment(node, source),
        _ => None,
    }
}

fn java_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "class_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        "interface_declaration" | "annotation_type_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Interface))
        }
        "enum_declaration" | "record_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Struct))
        }
        "method_declaration" | "constructor_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "field_declaration" => find_java_field_name(node, source),
        _ => None,
    }
}

fn javascript_like_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
        "export_statement" => javascript_default_export_symbol(node, source),
        "function_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Function))
        }
        "function_expression" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Function))
        }
        "class_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        "method_definition" => javascript_method_symbol(node, source),
        "public_field_definition" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "pair" => javascript_object_pair_symbol(node, source),
        "interface_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Interface))
        }
        "assignment_expression" => javascript_assignment_symbol(node, source),
        _ => None,
    }
}

fn javascript_default_export_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let text = node.utf8_text(source).ok()?.trim_start();
    let mut parts = text.split_whitespace();
    if parts.next()? != "export" || parts.next()? != "default" {
        return None;
    }

    Some(("default".to_string(), SymbolKind::Function))
}

fn javascript_method_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    if let Some(property) = child_text(node, "name", source)
        .and_then(|name| clean_js_property_key(&name).map(|property| property.to_string()))
        && let Some(object) = object_literal_context_name(node, source)
    {
        return Some((format!("{object}.{property}"), SymbolKind::Method));
    }

    child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
}

fn javascript_object_pair_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let key = node
        .child_by_field_name("key")
        .or_else(|| node.child(0))
        .and_then(|child| child.utf8_text(source).ok())?;
    let property = clean_js_property_key(key)?;
    let value = node
        .child_by_field_name("value")
        .and_then(|child| child.utf8_text(source).ok())?
        .trim();
    if !is_js_function_value(value) {
        return None;
    }

    let name = object_literal_context_name(node, source)
        .map(|object| format!("{object}.{property}"))
        .unwrap_or_else(|| property.to_string());
    Some((name, SymbolKind::Method))
}

fn javascript_variable_declarator_symbols(
    node: Node<'_>,
    source: &[u8],
) -> Vec<(String, SymbolKind)> {
    let Some(name_node) = node.child_by_field_name("name").or_else(|| node.child(0)) else {
        return Vec::new();
    };
    let value = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("right"))
        .and_then(|child| child.utf8_text(source).ok())
        .unwrap_or_default()
        .trim();

    if let Ok(name) = name_node.utf8_text(source)
        && is_js_identifier(name.trim())
    {
        let kind = if is_js_function_value(value) {
            SymbolKind::Function
        } else {
            SymbolKind::Variable
        };
        return vec![(name.trim().to_string(), kind)];
    }

    let mut symbols = Vec::new();
    collect_js_binding_symbols(name_node, source, SymbolKind::Variable, &mut symbols);
    symbols
}

fn collect_js_binding_symbols(
    node: Node<'_>,
    source: &[u8],
    fallback_kind: SymbolKind,
    symbols: &mut Vec<(String, SymbolKind)>,
) {
    match node.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => {
            if let Ok(name) = node.utf8_text(source)
                && is_js_identifier(name.trim())
                && !symbols.iter().any(|(existing, _)| existing == name.trim())
            {
                symbols.push((name.trim().to_string(), fallback_kind));
            }
        }
        "pair_pattern" => {
            if let Some(value) = node.child_by_field_name("value") {
                collect_js_binding_symbols(value, source, fallback_kind, symbols);
            } else if let Some(key) = node.child_by_field_name("key") {
                collect_js_binding_symbols(key, source, fallback_kind, symbols);
            }
        }
        "assignment_pattern" => {
            let field_default_is_function = node
                .child_by_field_name("right")
                .or_else(|| node.child_by_field_name("value"))
                .and_then(|child| child.utf8_text(source).ok())
                .is_some_and(is_js_function_value);
            let text_default_is_function = node
                .utf8_text(source)
                .ok()
                .and_then(|text| {
                    let equals = top_level_index(text, '=')?;
                    Some(text[equals + 1..].trim())
                })
                .is_some_and(is_js_function_value);
            let kind = if field_default_is_function || text_default_is_function {
                SymbolKind::Function
            } else {
                fallback_kind
            };
            if let Some(left) = node.child_by_field_name("left").or_else(|| node.child(0)) {
                collect_js_binding_symbols(left, source, kind, symbols);
            }
        }
        "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node_children(&mut cursor)
                .into_iter()
                .filter(|child| child.is_named())
            {
                collect_js_binding_symbols(child, source, fallback_kind.clone(), symbols);
            }
        }
        "object_pattern" | "array_pattern" => {
            let mut cursor = node.walk();
            for child in node_children(&mut cursor)
                .into_iter()
                .filter(|child| child.is_named())
            {
                collect_js_binding_symbols(child, source, fallback_kind.clone(), symbols);
            }
        }
        _ => {
            if let Ok(text) = node.utf8_text(source)
                && let Some(equals) = top_level_index(text, '=')
                && is_js_function_value(text[equals + 1..].trim())
                && let Some(left) = node.child(0)
            {
                collect_js_binding_symbols(left, source, SymbolKind::Function, symbols);
                return;
            }

            let mut cursor = node.walk();
            for child in node_children(&mut cursor)
                .into_iter()
                .filter(|child| child.is_named())
            {
                collect_js_binding_symbols(child, source, fallback_kind.clone(), symbols);
            }
        }
    }
}

fn javascript_assignment_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let left = node
        .child_by_field_name("left")
        .or_else(|| node.child(0))
        .and_then(|child| child.utf8_text(source).ok())?
        .trim();
    let right = node
        .child_by_field_name("right")
        .or_else(|| node.child(2))
        .and_then(|child| child.utf8_text(source).ok())
        .unwrap_or_default()
        .trim();

    let simple_function_name = simple_function_assignment_name(left, right);
    let name = commonjs_assignment_name(left, right)
        .or_else(|| computed_assignment_name(left, right))
        .or_else(|| simple_function_name.clone())?;
    let kind = if simple_function_name.as_deref() == Some(name.as_str()) {
        SymbolKind::Function
    } else if is_js_function_value(right) {
        SymbolKind::Method
    } else {
        SymbolKind::Variable
    };
    Some((name, kind))
}

fn commonjs_assignment_name(left: &str, right: &str) -> Option<String> {
    let left = left.trim();
    if left == "module.exports" || left == "exports" {
        return assignment_export_name(left, right);
    }

    if let Some(name) = left.strip_prefix("exports.") {
        return clean_js_property_name(name).map(|property| format!("exports.{property}"));
    }

    if let Some(name) = left.strip_prefix("module.exports.") {
        return clean_js_property_name(name).map(|property| format!("module.exports.{property}"));
    }

    if let Some((object, property)) = left.rsplit_once('.') {
        let object = object.trim();
        if is_js_identifier(object) {
            return clean_js_property_name(property).map(|property| format!("{object}.{property}"));
        }
    }

    None
}

fn computed_assignment_name(left: &str, right: &str) -> Option<String> {
    if !is_js_function_value(right) {
        return None;
    }

    let open = left.find('[')?;
    let close = left.rfind(']')?;
    if close <= open {
        return None;
    }

    let object = left[..open].trim();
    let property = left[open + 1..close].trim();
    if !is_js_identifier(object) || !is_js_identifier(property) {
        return None;
    }

    Some(format!("{object}.<dynamic>"))
}

fn simple_function_assignment_name(left: &str, right: &str) -> Option<String> {
    if is_js_identifier(left.trim()) && is_js_function_value(right) {
        Some(left.trim().to_string())
    } else {
        None
    }
}

fn is_js_function_value(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("function")
        || value.starts_with("async function")
        || value.starts_with("async ")
        || value.contains("=>")
}

fn assignment_export_name(left: &str, right: &str) -> Option<String> {
    if let Some(function_rest) = right.strip_prefix("function") {
        let name = function_rest
            .trim_start()
            .split(|ch: char| ch == '(' || ch.is_whitespace())
            .find(|part| !part.is_empty());
        return name
            .filter(|candidate| is_js_identifier(candidate))
            .map(ToOwned::to_owned)
            .or_else(|| Some(left.to_string()));
    }

    if let Some(identifier) = right.split_whitespace().next()
        && is_js_identifier(identifier)
    {
        return Some(identifier.to_string());
    }

    Some(left.to_string())
}

fn clean_js_property_name(property: &str) -> Option<&str> {
    let property = property.trim();
    if is_js_identifier(property) {
        Some(property)
    } else {
        None
    }
}

fn clean_js_property_key(property: &str) -> Option<&str> {
    let property = property.trim();
    if is_js_identifier(property) {
        return Some(property);
    }

    let value = string_literal_value(property)?;
    if is_js_identifier(&value) {
        Some(property[1..property.len() - 1].trim())
    } else {
        None
    }
}

fn object_literal_context_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "object" {
            return object_literal_owner_name(parent, source);
        }
        current = parent.parent();
    }

    None
}

fn object_literal_owner_name(object: Node<'_>, source: &[u8]) -> Option<String> {
    let parent = object.parent()?;
    match parent.kind() {
        "variable_declarator" => child_text(parent, "name", source)
            .filter(|name| is_js_identifier(name))
            .or_else(|| {
                parent
                    .child(0)?
                    .utf8_text(source)
                    .ok()
                    .map(ToOwned::to_owned)
            }),
        "assignment_expression" => {
            let left = parent
                .child_by_field_name("left")
                .or_else(|| parent.child(0))
                .and_then(|child| child.utf8_text(source).ok())?
                .trim();
            if is_js_identifier(left) {
                Some(left.to_string())
            } else {
                commonjs_assignment_name(left, "{}")
            }
        }
        "pair" => {
            let property = parent
                .child_by_field_name("key")
                .or_else(|| parent.child(0))
                .and_then(|child| child.utf8_text(source).ok())
                .and_then(clean_js_property_key)?;
            let object = object_literal_context_name(parent, source)?;
            Some(format!("{object}.{property}"))
        }
        _ => None,
    }
}

fn is_js_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn child_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source).ok())
        .map(ToOwned::to_owned)
}

fn first_child_text(node: Node<'_>, source: &[u8], kinds: &[&str]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if kinds.contains(&child.kind()) {
            return child.utf8_text(source).ok().map(ToOwned::to_owned);
        }
    }
    None
}

fn find_go_type_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "type_spec" {
            let name = child_text(child, "name", source)?;
            let kind = if child
                .child_by_field_name("type")
                .is_some_and(|type_node| matches!(type_node.kind(), "struct_type"))
            {
                SymbolKind::Struct
            } else {
                SymbolKind::Interface
            };
            return Some((name, kind));
        }
    }
    None
}

fn find_go_value_name(
    node: Node<'_>,
    source: &[u8],
    kind: SymbolKind,
) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if matches!(child.kind(), "const_spec" | "var_spec") {
            let name = child_text(child, "name", source)?;
            return Some((name, kind));
        }
    }
    None
}

fn find_java_field_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child_text(child, "name", source)?;
            return Some((name, SymbolKind::Variable));
        }
    }
    None
}

fn find_csharp_field_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "variable_declaration"
            && let Some(name) = find_csharp_variable_name(child, source)
        {
            return Some((name, SymbolKind::Variable));
        }
    }
    None
}

fn find_csharp_variable_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name) = child_text(child, "name", source) {
                return Some(name);
            }
            if let Some(name) = find_c_declarator_identifier(child, source) {
                return Some(name);
            }
        }
    }
    None
}

fn find_php_property_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "property_element" {
            let name = child_text(child, "name", source)?
                .trim_start_matches('$')
                .to_string();
            return Some((name, SymbolKind::Variable));
        }
    }
    None
}

fn find_php_const_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() == "const_element" {
            let name = first_child_text(child, source, &["name"])?;
            return Some((name, SymbolKind::Constant));
        }
    }
    None
}

fn ruby_string_argument_targets(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    collect_string_content(arguments, source, &mut targets);
    targets
}

fn collect_string_content(node: Node<'_>, source: &[u8], targets: &mut Vec<String>) {
    if node.kind() == "string_content"
        && let Ok(text) = node.utf8_text(source)
        && !text.trim().is_empty()
    {
        targets.push(text.trim().to_string());
    }

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        collect_string_content(child, source, targets);
    }
}

fn find_ruby_constant_assignment(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let left = node.child_by_field_name("left")?;
    if matches!(left.kind(), "constant" | "scope_resolution") {
        return left
            .utf8_text(source)
            .ok()
            .map(|name| (name.to_string(), SymbolKind::Constant));
    }
    None
}

fn find_c_function_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    find_c_declarator_identifier(node.child_by_field_name("declarator")?, source)
}

fn find_c_function_declaration_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let declarator = node.child_by_field_name("declarator")?;
    if declarator.kind() != "function_declarator" {
        return None;
    }

    declarator
        .child_by_field_name("declarator")
        .filter(|child| matches!(child.kind(), "identifier" | "field_identifier"))
        .and_then(|child| child.utf8_text(source).ok())
        .map(ToOwned::to_owned)
}

fn find_c_typedef_name(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    let declarator = node.child_by_field_name("declarator").or_else(|| {
        let mut cursor = node.walk();
        node_children(&mut cursor)
            .into_iter()
            .find(|child| child.kind().contains("declarator"))
    })?;
    let name = find_c_declarator_identifier(declarator, source)?;
    let kind = if node
        .child_by_field_name("type")
        .is_some_and(|type_node| matches!(type_node.kind(), "struct_specifier" | "class_specifier"))
    {
        SymbolKind::Struct
    } else {
        SymbolKind::Interface
    };
    Some((name, kind))
}

fn find_c_declarator_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if matches!(node.kind(), "identifier" | "field_identifier") {
        return node.utf8_text(source).ok().map(ToOwned::to_owned);
    }

    if let Some(declarator) = node.child_by_field_name("declarator")
        && let Some(name) = find_c_declarator_identifier(declarator, source)
    {
        return Some(name);
    }

    if let Some(name) = child_text(node, "name", source) {
        return Some(name);
    }

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if let Some(name) = find_c_declarator_identifier(child, source) {
            return Some(name);
        }
    }

    None
}

fn is_inside_class(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_definition" {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn is_inside_rust_impl(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "impl_item" {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn should_enter(path: &Path) -> bool {
    let ignored: HashSet<&str> = [
        ".git",
        ".codeinsight",
        "node_modules",
        "target",
        "dist",
        "build",
        ".venv",
        "vendor",
        ".next",
        ".turbo",
    ]
    .into_iter()
    .collect();

    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| !ignored.contains(name))
}

#[allow(dead_code)]
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pnpm_workspace_catalog_metadata() {
        let workspace = r#"
packages:
  - "packages/**"
catalog:
  default-catalog-ui: ^1.0.0
  ignored-default-ui: ^2.0.0 # comment
catalogs:
  react18:
    catalog-ui: ^18.0.0
  react17:
    catalog-ui: ^17.0.0
"#;

        assert!(pnpm_workspace_catalog_declares_dependency(
            workspace,
            "default",
            "default-catalog-ui"
        ));
        assert!(pnpm_workspace_catalog_declares_dependency(
            workspace,
            "react18",
            "catalog-ui"
        ));
        assert!(pnpm_workspace_catalog_declares_dependency(
            workspace,
            "react17",
            "catalog-ui"
        ));
        assert!(!pnpm_workspace_catalog_declares_dependency(
            workspace,
            "react18",
            "default-catalog-ui"
        ));
        assert!(!pnpm_workspace_catalog_declares_dependency(
            workspace,
            "missing",
            "catalog-ui"
        ));
    }

    #[test]
    fn parses_catalog_protocol_names() {
        assert_eq!(catalog_protocol_name("catalog:"), Some("default"));
        assert_eq!(catalog_protocol_name("catalog:react18"), Some("react18"));
        assert_eq!(catalog_protocol_name("^1.0.0"), None);
    }

    #[test]
    fn parses_package_export_array_targets_in_order() {
        let exports = serde_json::json!({
            "./feature": {
                "import": [
                    null,
                    "external-lib",
                    "./dist/missing-feature.js",
                    "./dist/feature.js"
                ],
                "default": "./dist/default-feature.js"
            }
        });

        assert_eq!(
            package_export_mappings(&exports, "./feature", &default_package_conditions()),
            Some(vec![
                PathBuf::from("./dist/missing-feature.js"),
                PathBuf::from("./dist/feature.js")
            ])
        );
    }

    #[test]
    fn parses_package_export_targets_with_custom_conditions() {
        let exports = serde_json::json!({
            ".": {
                "types": "./dist/index.d.ts",
                "import": "./dist/index.js",
                "default": "./dist/default.js"
            }
        });
        let conditions = vec![
            "types".to_string(),
            "import".to_string(),
            "default".to_string(),
        ];

        assert_eq!(
            package_export_mappings(&exports, ".", &conditions),
            Some(vec![PathBuf::from("./dist/index.d.ts")])
        );
    }

    #[test]
    fn prefers_exact_package_export_mapping_before_wildcards() {
        let exports = serde_json::json!({
            "./feature/*": "./dist/wildcard/*.js",
            "./feature/special": "./dist/special.js",
            "./feature/special/*": "./dist/special/*.js"
        });

        assert_eq!(
            package_export_mappings(&exports, "./feature/special", &default_package_conditions()),
            Some(vec![PathBuf::from("./dist/special.js")])
        );
        assert_eq!(
            package_export_mappings(
                &exports,
                "./feature/special/button",
                &default_package_conditions()
            ),
            Some(vec![PathBuf::from("./dist/special/button.js")])
        );
    }

    #[test]
    fn prefers_exact_package_import_mapping_before_wildcards() {
        let imports = serde_json::json!({
            "#feature/*": "./src/wildcard/*.ts",
            "#feature/special": "./src/special.ts",
            "#feature/special/*": "./src/special/*.ts"
        });

        assert_eq!(
            package_import_mappings(&imports, "#feature/special", &default_package_conditions()),
            Some(vec![PathBuf::from("./src/special.ts")])
        );
        assert_eq!(
            package_import_mappings(
                &imports,
                "#feature/special/button",
                &default_package_conditions()
            ),
            Some(vec![PathBuf::from("./src/special/button.ts")])
        );
    }

    #[test]
    fn parses_null_package_export_as_disabled_mapping() {
        let exports = serde_json::json!({
            "./disabled": null,
            "./conditional": {
                "import": null,
                "default": "./dist/conditional-fallback.js"
            },
            "./conditional-external": {
                "import": "external-lib",
                "default": "./dist/conditional-external-fallback.js"
            },
            "./enabled": "./dist/enabled.js"
        });

        assert_eq!(
            package_export_mappings(&exports, "./disabled", &default_package_conditions()),
            Some(Vec::new())
        );
        assert_eq!(
            package_export_mappings(&exports, "./missing", &default_package_conditions()),
            None
        );
        assert_eq!(
            package_export_mappings(&exports, "./conditional", &default_package_conditions()),
            Some(Vec::new())
        );
        assert_eq!(
            package_export_mappings(
                &exports,
                "./conditional-external",
                &default_package_conditions()
            ),
            Some(Vec::new())
        );
        assert_eq!(
            package_export_mappings(
                &exports,
                "./conditional",
                &["browser".to_string(), "default".to_string()]
            ),
            Some(vec![PathBuf::from("./dist/conditional-fallback.js")])
        );
    }

    #[test]
    fn parses_null_package_import_as_disabled_mapping() {
        let imports = serde_json::json!({
            "#disabled": null,
            "#conditional": {
                "import": null,
                "default": "./src/conditional-fallback.ts"
            },
            "#conditional-external": {
                "import": "external-import",
                "default": "./src/conditional-external-fallback.ts"
            },
            "#array": [
                null,
                "external-import",
                "./src/array-fallback.ts"
            ],
            "#enabled/*": "./src/*.ts"
        });

        assert_eq!(
            package_import_mappings(&imports, "#disabled", &default_package_conditions()),
            Some(Vec::new())
        );
        assert_eq!(
            package_import_mappings(&imports, "#missing", &default_package_conditions()),
            None
        );
        assert_eq!(
            package_import_mappings(&imports, "#conditional", &default_package_conditions()),
            Some(Vec::new())
        );
        assert_eq!(
            package_import_mappings(
                &imports,
                "#conditional-external",
                &default_package_conditions()
            ),
            Some(Vec::new())
        );
        assert_eq!(
            package_import_mappings(
                &imports,
                "#conditional",
                &["browser".to_string(), "default".to_string()]
            ),
            Some(vec![PathBuf::from("./src/conditional-fallback.ts")])
        );
        assert_eq!(
            package_import_mappings(&imports, "#array", &default_package_conditions()),
            Some(vec![PathBuf::from("./src/array-fallback.ts")])
        );
        assert_eq!(
            package_import_mappings(&imports, "#enabled/button", &default_package_conditions()),
            Some(vec![PathBuf::from("./src/button.ts")])
        );
    }

    #[test]
    fn captures_and_applies_multiple_path_wildcards() {
        let captures =
            path_pattern_captures("@scope/*/component/*", "@scope/admin/component/button").unwrap();

        assert_eq!(captures, vec!["admin".to_string(), "button".to_string()]);
        assert_eq!(
            apply_path_mapping("src/*/component/*.ts", &captures),
            Some(PathBuf::from("src/admin/component/button.ts"))
        );
        assert_eq!(apply_path_mapping("src/*.ts", &captures), None);
    }

    #[test]
    fn prefers_exact_path_pattern_mapping_before_wildcards() {
        let mappings = serde_json::json!({
            "@app/*": ["*"],
            "@app/special": ["special.ts"],
            "@app/special/*": ["special/*.ts"]
        });
        let entries = mappings.as_object().unwrap();

        assert_eq!(
            path_pattern_mapping(entries, "@app/special"),
            Some((entries.get("@app/special").unwrap(), Vec::<String>::new()))
        );
        assert_eq!(
            path_pattern_mapping(entries, "@app/special/button"),
            Some((
                entries.get("@app/special/*").unwrap(),
                vec!["button".to_string()]
            ))
        );
    }

    #[test]
    fn applies_package_browser_mappings() {
        let browser_string_package = serde_json::json!({
            "main": "./dist/node.js",
            "browser": "./dist/browser.js"
        });
        assert_eq!(
            package_browser_mappings(
                &browser_string_package,
                ".",
                &[PathBuf::from("./dist/node.js")]
            ),
            vec![PathBuf::from("./dist/browser.js")]
        );
        let external_browser_string_package = serde_json::json!({
            "main": "./dist/node.js",
            "browser": "external-browser-entry"
        });
        assert!(
            package_browser_mappings(
                &external_browser_string_package,
                ".",
                &[PathBuf::from("./dist/node.js")]
            )
            .is_empty()
        );

        let browser_object_package = serde_json::json!({
            "browser": {
                "./dist/server.js": "./dist/browser-server.js",
                "dist/plain.js": "./dist/browser-plain.js",
                "./dist/external.js": "external-browser-shim",
                "./dist/absolute.js": "/dist/browser-absolute.js",
                "./dist/object.js": {
                    "browser": "./dist/browser-object.js"
                },
                "./dist/disabled.js": false
            }
        });
        assert_eq!(
            package_browser_mappings(
                &browser_object_package,
                "./server",
                &[PathBuf::from("./dist/server.js")]
            ),
            vec![PathBuf::from("./dist/browser-server.js")]
        );
        assert_eq!(
            package_browser_mappings(
                &browser_object_package,
                "./plain",
                &[PathBuf::from("./dist/plain.js")]
            ),
            vec![PathBuf::from("./dist/browser-plain.js")]
        );
        assert!(
            package_browser_mappings(
                &browser_object_package,
                "./external",
                &[PathBuf::from("./dist/external.js")]
            )
            .is_empty()
        );
        assert!(
            package_browser_mappings(
                &browser_object_package,
                "./absolute",
                &[PathBuf::from("./dist/absolute.js")]
            )
            .is_empty()
        );
        assert!(
            package_browser_mappings(
                &browser_object_package,
                "./object",
                &[PathBuf::from("./dist/object.js")]
            )
            .is_empty()
        );
        assert!(
            package_browser_mappings(
                &browser_object_package,
                "./disabled",
                &[PathBuf::from("./dist/disabled.js")]
            )
            .is_empty()
        );
    }

    #[test]
    fn parses_package_metadata_entries_as_local_targets() {
        let package = serde_json::json!({
            "module": "external-entry",
            "main": "/dist/main.js",
            "types": "./dist/index.d.ts"
        });
        assert_eq!(
            package_metadata_entry(&package, "."),
            Some(PathBuf::from("./dist/index.d.ts"))
        );

        let invalid_package = serde_json::json!({
            "module": "external-entry",
            "main": "/dist/main.js",
            "types": {
                "default": "./dist/index.d.ts"
            },
            "typings": false
        });
        assert_eq!(package_metadata_entry(&invalid_package, "."), None);
        assert_eq!(
            package_metadata_entry(&invalid_package, "./feature"),
            Some(PathBuf::from("feature"))
        );
    }

    #[test]
    fn extracts_python_symbols() {
        let source = r#"
class AuthService:
    def login(self):
        pass

def helper():
    pass
"#;
        let symbols = extract_symbols(source, Language::Python, "auth.py").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"AuthService.login"));
        assert!(names.contains(&"helper"));
    }

    #[test]
    fn extracts_typescript_symbols() {
        let source = r#"
export interface UserRepo {}
export interface DetailedRepo extends UserRepo, Cache<User, Role> {}
export class AuthService extends BaseAuthService implements UserRepo {
  login() {}
}
export function helper() {}
const token = "x";
"#;
        let symbols = extract_symbols(source, Language::TypeScript, "auth.ts").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"UserRepo"));
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"AuthService.login"));
        assert!(names.contains(&"helper"));
        assert!(names.contains(&"token"));

        let deps = extract_dependencies(source, Language::TypeScript, "auth.ts").unwrap();
        assert!(deps.iter().any(|dependency| {
            dependency.target == "UserRepo"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("DetailedRepo")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "Cache"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("DetailedRepo")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));
        assert!(deps.iter().all(|dependency| dependency.target != "Role>"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "BaseAuthService"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "UserRepo"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));
    }

    #[test]
    fn extracts_commonjs_assignment_symbols() {
        let source = r#"
module.exports = View;
exports.normalizeType = function(type) {
  return type;
};
app.use = function use(fn) {
  return fn;
};
module.exports.create = function create() {};
methods.forEach(function (method) {
  app[method] = function (path) {
    return path;
  };
});
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "express.js").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"View"));
        assert!(names.contains(&"exports.normalizeType"));
        assert!(names.contains(&"app.use"));
        assert!(names.contains(&"module.exports.create"));
        assert!(names.contains(&"app.<dynamic>"));
    }

    #[test]
    fn extracts_javascript_function_value_symbols() {
        let source = r#"
const namedHandler = function handleNamed(req, res) {
  return res;
};
const arrowHandler = (req, res) => res;
const count = 1, inlineHandler = async (req) => req;
handler = () => true;
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "handlers.js").unwrap();
        let kind_for = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.qualified_name == name)
                .map(|symbol| symbol.kind.clone())
        };

        assert_eq!(kind_for("namedHandler"), Some(SymbolKind::Function));
        assert_eq!(kind_for("handleNamed"), Some(SymbolKind::Function));
        assert_eq!(kind_for("arrowHandler"), Some(SymbolKind::Function));
        assert_eq!(kind_for("inlineHandler"), Some(SymbolKind::Function));
        assert_eq!(kind_for("handler"), Some(SymbolKind::Function));
        assert_eq!(kind_for("count"), Some(SymbolKind::Variable));
    }

    #[test]
    fn extracts_javascript_object_literal_function_symbols() {
        let source = r#"
const handlers = {
  getUser(req, res) {
    return res;
  },
  saveUser: (req, res) => res,
  removeUser: function removeUserImpl(req, res) {
    return res;
  },
  count: 1,
  nested: {
    ping() {
      return true;
    }
  }
};
module.exports = {
  create() {}
};
exports.tools = {
  run: async () => {}
};
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "handlers.js").unwrap();
        let kind_for = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.qualified_name == name)
                .map(|symbol| symbol.kind.clone())
        };

        assert_eq!(kind_for("handlers.getUser"), Some(SymbolKind::Method));
        assert_eq!(kind_for("handlers.saveUser"), Some(SymbolKind::Method));
        assert_eq!(kind_for("handlers.removeUser"), Some(SymbolKind::Method));
        assert_eq!(kind_for("removeUserImpl"), Some(SymbolKind::Function));
        assert_eq!(kind_for("handlers.nested.ping"), Some(SymbolKind::Method));
        assert_eq!(kind_for("module.exports.create"), Some(SymbolKind::Method));
        assert_eq!(kind_for("exports.tools.run"), Some(SymbolKind::Method));
        assert_eq!(kind_for("handlers.count"), None);
    }

    #[test]
    fn extracts_javascript_destructured_binding_symbols() {
        let source = r#"
const { handler, getUser: userHandler, nested: { saveUser }, list = () => true, ...rest } = controllers;
const [firstHandler, , thirdHandler] = handlers;
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "bindings.js").unwrap();
        let kind_for = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.qualified_name == name)
                .map(|symbol| symbol.kind.clone())
        };

        assert_eq!(kind_for("handler"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("userHandler"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("saveUser"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("list"), Some(SymbolKind::Function));
        assert_eq!(kind_for("rest"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("firstHandler"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("thirdHandler"), Some(SymbolKind::Variable));
        assert_eq!(kind_for("getUser"), None);
        assert_eq!(kind_for("nested"), None);
    }

    #[test]
    fn extracts_javascript_default_export_symbol() {
        let source = r#"
export default function renderDefault() {
  return "ok";
}
"#;
        let symbols = extract_symbols(source, Language::TypeScript, "ui.ts").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"default"));
        assert!(names.contains(&"renderDefault"));
    }

    #[test]
    fn extracts_bash_symbols_dependencies_and_calls() {
        let source = r#"
source ./lib/common.sh
. ../shared/env.bash
source "$DYNAMIC_ROOT/runtime.sh"

bootstrap() {
  load_env
  ./scripts/build.sh
}

load_env() {
  echo ready
}
"#;
        let symbols = extract_symbols(source, Language::Bash, "scripts/bootstrap.sh").unwrap();
        assert!(
            symbols.iter().any(|symbol| {
                symbol.name == "bootstrap" && symbol.kind == SymbolKind::Function
            })
        );

        let deps = extract_dependencies(source, Language::Bash, "scripts/bootstrap.sh").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"./lib/common.sh"));
        assert!(targets.contains(&"../shared/env.bash"));
        assert!(
            !targets.contains(&"$DYNAMIC_ROOT/runtime.sh"),
            "dynamic source targets should not become deterministic dependencies"
        );
        assert!(deps.iter().all(|dependency| dependency.kind == "source"));

        let calls = extract_calls(source, Language::Bash, "scripts/bootstrap.sh", &symbols);
        assert!(calls.iter().any(|call| {
            call.caller == "bootstrap"
                && call.callee == "load_env"
                && call.callee_file.as_deref() == Some("scripts/bootstrap.sh")
        }));
        assert!(
            calls
                .iter()
                .any(|call| { call.caller == "bootstrap" && call.callee == "build.sh" })
        );
    }

    #[test]
    fn resolves_bash_source_dependencies() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("scripts/lib")).unwrap();
        std::fs::write(
            dir.path().join("scripts/bootstrap.sh"),
            "source ./lib/common.sh\nbootstrap() {\n  run_common\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("scripts/lib/common.sh"),
            "run_common() {\n  echo ok\n}\n",
        )
        .unwrap();

        let report = index_project(dir.path(), false).unwrap();
        assert_eq!(report.errors.len(), 0);

        let store = Store::open(dir.path()).unwrap();
        let graph = store
            .dependency_graph(dir.path(), 10, 0, &[], &[], &[])
            .unwrap();
        assert!(graph.dependencies.iter().any(|dependency| {
            dependency.source_file == "scripts/bootstrap.sh"
                && dependency.target == "./lib/common.sh"
                && dependency.kind == "source"
                && dependency.resolved_file.as_deref() == Some("scripts/lib/common.sh")
        }));

        let calls = store.callees("bootstrap", 10).unwrap();
        assert!(calls.iter().any(|call| {
            call.callee == "run_common"
                && call.callee_file.as_deref() == Some("scripts/lib/common.sh")
        }));
    }

    #[test]
    fn extracts_go_symbols() {
        let source = r#"
package auth

type Service struct {}

func NewService() {}

func (s *Service) Login() {}
"#;
        let symbols = extract_symbols(source, Language::Go, "auth.go").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"Service"));
        assert!(names.contains(&"NewService"));
        assert!(names.contains(&"Login"));
    }

    #[test]
    fn parses_go_import_aliases() {
        assert_eq!(
            go_import_specs(
                r#"
import (
  "fmt"
  authsvc "github.com/example/codeinsight/internal/auth"
  _ "github.com/example/codeinsight/internal/sideeffect"
)
"#
            ),
            vec![
                ("fmt".to_string(), Some("fmt".to_string())),
                (
                    "github.com/example/codeinsight/internal/auth".to_string(),
                    Some("authsvc".to_string())
                ),
                (
                    "github.com/example/codeinsight/internal/sideeffect".to_string(),
                    None
                ),
            ]
        );
        assert_eq!(
            go_import_specs(r#"import "github.com/example/codeinsight/internal/config""#),
            vec![(
                "github.com/example/codeinsight/internal/config".to_string(),
                Some("config".to_string())
            )]
        );
    }

    #[test]
    fn normalizes_go_package_calls() {
        let source = r#"
package main

func main() {
    auth.Login()
    config.Load()
}
"#;
        let symbols = extract_symbols(source, Language::Go, "main.go").unwrap();
        let calls = extract_calls(source, Language::Go, "main.go", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();
        assert!(callees.contains(&"auth.Login"));
        assert!(callees.contains(&"config.Load"));
    }

    #[test]
    fn parses_go_module_path() {
        let text = r#"
// comment
module github.com/example/codeinsight // inline comment

go 1.22
"#;
        assert_eq!(
            parse_go_module_path(text).as_deref(),
            Some("github.com/example/codeinsight")
        );
        assert_eq!(parse_go_module_path("modulefoo invalid"), None);
    }

    #[test]
    fn extracts_c_symbols_dependencies_and_calls() {
        let source = r#"
#include "auth.h"
#include <stdio.h>
#define AUTH_MAX 8

typedef struct AuthService {
  int count;
} AuthService;

int helper(int value) {
  return value + 1;
}

int declared_helper(int value);

int login(AuthService *service) {
  return helper(declared_helper(service->count));
}
"#;
        let symbols = extract_symbols(source, Language::C, "auth.c").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AUTH_MAX"));
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"helper"));
        assert!(names.contains(&"declared_helper"));
        assert!(names.contains(&"login"));

        let deps = extract_dependencies(source, Language::C, "auth.c").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"auth.h"));
        assert!(targets.contains(&"<stdio.h>"));

        let calls = extract_calls(source, Language::C, "auth.c", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "login" && call.callee == "helper")
        );
    }

    #[test]
    fn extracts_cpp_symbols_dependencies_and_calls() {
        let source = r#"
#include "auth.hpp"
#include <string>

class AuthService {
public:
  int login() {
    return helper();
  }

private:
  int helper() {
    return 1;
  }
};

int make_service() {
  AuthService service;
  return service.login();
}
"#;
        let symbols = extract_symbols(source, Language::Cpp, "auth.cpp").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"AuthService.login"));
        assert!(names.contains(&"AuthService.helper"));
        assert!(names.contains(&"make_service"));

        let deps = extract_dependencies(source, Language::Cpp, "auth.cpp").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"auth.hpp"));
        assert!(targets.contains(&"<string>"));

        let calls = extract_calls(source, Language::Cpp, "auth.cpp", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "AuthService.login" && call.callee == "helper")
        );
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "make_service" && call.callee == "login")
        );
    }

    #[test]
    fn extracts_csharp_symbols_dependencies_and_calls() {
        let source = r#"
using System;
using Alias = System.Text.StringBuilder;
using static System.Math;

namespace Example.Auth;

public class AuthService : Example.Auth.BaseAuthService, IUserService {
    private string token;

    public AuthService(string token) {
        this.token = token;
    }

    public int Count { get; set; }

    public ProfileService Profile { get; } = new();

    public bool Login(User user) {
        Audit(user);
        return true;
    }

    private void Audit(User user) {}
}

public interface UserRepository {
    User Find(string id);
}

public interface IUserService {}

public class ProfileService {
    public string Load(string id) => id;
}

public class BaseAuthService {
    protected string BaseTag() => "base";
}
"#;
        let symbols = extract_symbols(source, Language::CSharp, "AuthService.cs").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"AuthService.token"));
        assert!(names.contains(&"AuthService.AuthService"));
        assert!(names.contains(&"AuthService.Count"));
        assert!(names.contains(&"AuthService.Login"));
        assert!(names.contains(&"AuthService.Audit"));
        assert!(names.contains(&"UserRepository"));
        assert!(names.contains(&"UserRepository.Find"));

        let deps = extract_dependencies(source, Language::CSharp, "AuthService.cs").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"System"));
        assert!(targets.contains(&"System.Text.StringBuilder"));
        assert!(targets.contains(&"System.Math"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "Example.Auth.BaseAuthService"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "ProfileService"
                && dependency.kind == "property_type"
                && dependency.local_alias.as_deref() == Some("Profile")
                && dependency.imported_symbol.as_deref() == Some("AuthService")
        }));
        assert!(deps.iter().all(
            |dependency| dependency.target != "IUserService" || dependency.kind != "base_type"
        ));

        let calls = extract_calls(source, Language::CSharp, "AuthService.cs", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "AuthService.Login" && call.callee == "Audit")
        );
    }

    #[test]
    fn parses_csharp_using_targets_with_kinds() {
        assert_eq!(
            csharp_using_target("using App.Services;"),
            Some(("App.Services".to_string(), "using"))
        );
        assert_eq!(
            csharp_using_target("using Alias = App.Support.AuditLog;"),
            Some(("App.Support.AuditLog".to_string(), "using_alias"))
        );
        assert_eq!(
            csharp_using_target("using static App.Support.MathUtil;"),
            Some(("App.Support.MathUtil".to_string(), "using_static"))
        );
        assert_eq!(csharp_using_target("using (resource) {}"), None);
    }

    #[test]
    fn parses_csharp_using_aliases() {
        assert_eq!(
            csharp_using_alias(
                "using Audit = App.Support.AuditLog;",
                "App.Support.AuditLog",
                "using_alias"
            ),
            (Some("Audit".to_string()), Some("*".to_string()))
        );
        assert_eq!(
            csharp_using_alias(
                "using static App.Support.MathUtil;",
                "App.Support.MathUtil",
                "using_static"
            ),
            (None, Some("*".to_string()))
        );
    }

    #[test]
    fn parses_csharp_namespace_targets() {
        assert_eq!(
            csharp_namespace_targets("namespace App.Controllers;"),
            vec!["App.Controllers".to_string()]
        );
        assert_eq!(
            csharp_namespace_targets("namespace App.Controllers {"),
            vec!["App.Controllers".to_string()]
        );
    }

    #[test]
    fn parses_csharp_extension_receiver_types() {
        assert_eq!(
            csharp_extension_receiver_type(
                "public static string FormatForDisplay(this UserService users, string id) { }"
            ),
            Some("UserService".to_string())
        );
        assert_eq!(
            csharp_extension_receiver_type(
                "public static string FormatForDisplay(this App.Services.UserService users, string id) { }"
            ),
            Some("App.Services.UserService".to_string())
        );
        assert_eq!(
            csharp_extension_receiver_type(
                "public static string FormatForDisplay(this List<UserService> users, string id) { }"
            ),
            Some("UserService".to_string())
        );
        assert_eq!(
            csharp_extension_receiver_type(
                "public static string FormatForDisplay(UserService users, string id) { }"
            ),
            None
        );
    }

    #[test]
    fn parses_csharp_type_bindings() {
        assert_eq!(
            csharp_type_bindings("private readonly UserService users;"),
            vec![("UserService".to_string(), "users".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings("App.Services.UserService users"),
            vec![(
                "App.Services.UserService".to_string(),
                "users".to_string(),
                None
            )]
        );
        assert_eq!(
            csharp_type_bindings("UserService? maybeUsers = users;"),
            vec![("UserService".to_string(), "maybeUsers".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings("UserService[] servicePool = new[] { users };"),
            vec![("UserService".to_string(), "servicePool".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings("List<UserService> listUsers = new();"),
            vec![("UserService".to_string(), "listUsers".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings(
                "System.Collections.Generic.IEnumerable<App.Services.UserService> enumerableUsers = [];"
            ),
            vec![(
                "App.Services.UserService".to_string(),
                "enumerableUsers".to_string(),
                None
            )]
        );
        assert_eq!(
            csharp_type_bindings("Dictionary<string, UserService> usersById = new();"),
            vec![("UserService".to_string(), "usersById".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings(
                "Dictionary<string, UserService> usersById = new() { [id] = users };"
            ),
            vec![("UserService".to_string(), "usersById".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings(
                "System.Collections.Generic.IReadOnlyDictionary<string, App.Services.UserService> readOnlyUsers = usersById;"
            ),
            vec![(
                "App.Services.UserService".to_string(),
                "readOnlyUsers".to_string(),
                None
            )]
        );
        assert_eq!(
            csharp_type_bindings("var createdUsers = new UserService();"),
            vec![("UserService".to_string(), "createdUsers".to_string(), None)]
        );
        assert_eq!(
            csharp_type_bindings("var createdBackupUsers = new App.Services.UserService();"),
            vec![(
                "App.Services.UserService".to_string(),
                "createdBackupUsers".to_string(),
                None
            )]
        );
        assert_eq!(
            csharp_type_bindings("Lazy<UserService> lazyUsers = new();"),
            vec![(
                "UserService".to_string(),
                "lazyUsers".to_string(),
                Some("Value".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("System.Lazy<App.Services.UserService> lazyUsers = new();"),
            vec![(
                "App.Services.UserService".to_string(),
                "lazyUsers".to_string(),
                Some("Value".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("Task<UserService> taskUsers = Task.FromResult(users);"),
            vec![(
                "UserService".to_string(),
                "taskUsers".to_string(),
                Some("Result".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("ValueTask<UserService> valueTaskUsers = new(users);"),
            vec![(
                "UserService".to_string(),
                "valueTaskUsers".to_string(),
                Some("Result".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("var lazyUsers = new Lazy<UserService>();"),
            vec![(
                "UserService".to_string(),
                "lazyUsers".to_string(),
                Some("Value".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("var taskUsers = new Task<UserService>(() => users);"),
            vec![(
                "UserService".to_string(),
                "taskUsers".to_string(),
                Some("Result".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings("var valueTaskUsers = new ValueTask<UserService>(users);"),
            vec![(
                "UserService".to_string(),
                "valueTaskUsers".to_string(),
                Some("Result".to_string())
            )]
        );
        assert_eq!(
            csharp_type_bindings(
                "var lazyUsers = new System.Lazy<App.Services.UserService>(() => users);"
            ),
            vec![(
                "App.Services.UserService".to_string(),
                "lazyUsers".to_string(),
                Some("Value".to_string())
            )]
        );
        assert!(csharp_type_bindings("string name").is_empty());
        assert!(csharp_type_bindings("Tuple<string, int, UserService> users").is_empty());
        assert!(
            csharp_type_bindings("Tuple<string, int, UserService> tupleUsers = new();").is_empty()
        );
        assert!(
            csharp_type_bindings("List<Dictionary<string, UserService>> nestedUsers = new();")
                .is_empty()
        );
        assert!(
            csharp_type_bindings(
                "Task<List<UserService>> taskListUsers = Task.FromResult(listUsers);"
            )
            .is_empty()
        );
        assert!(
            csharp_type_bindings(
                "Lazy<Dictionary<string, UserService>> lazyMappedUsers = new(() => usersById);"
            )
            .is_empty()
        );
        assert!(
            csharp_type_bindings(
                "var taskListUsers = new Task<List<UserService>>(() => listUsers);"
            )
            .is_empty()
        );
        assert!(
            csharp_type_bindings(
                "var lazyMappedUsers = new Lazy<Dictionary<string, UserService>>(() => usersById);"
            )
            .is_empty()
        );
        assert!(csharp_type_bindings("var users = GetUsers();").is_empty());
    }

    #[test]
    fn normalizes_csharp_qualified_calls() {
        let source = r#"
public class AuthController {
    public async Task<string> Login(string id) {
        Audit.Record(id);
        App.Support.AuditLog.Record(id);
        App.Support.MathUtil.ClampName(id);
        new UserService().Find(id);
        new App.Services.UserService().ExternalProfile.Load(id);
        new UserService { }.Find(id);
        new UserService { }.ExternalProfile.Load(id);
        new App.Services.UserService { }.ExternalProfile.Load(id);
        new UserService() { }.Find(id);
        new List<UserService> { users }[0].Find(id);
        new Dictionary<string, UserService> { [id] = users }[id].Find(id);
        new List<UserService> { users }[0].ExternalProfile.Load(id);
        new Lazy<UserService>(() => users).Value.Find(id);
        new Task<UserService>(() => users).Result.Find(id);
        new Lazy<UserService>(() => users).Value.ExternalProfile.Load(id);
        new ValueTask<UserService>(users).Result.Find(id);
        new System.Lazy<App.Services.UserService>(() => users).Value.Find(id);
        new System.Lazy<App.Services.UserService>(() => users).Value.ExternalProfile.Load(id);
        this.users.Find(id);
        users?.Find(id);
        users!.Find(id);
        this.users?.Find(id);
        this.users!.Find(id);
        await users.FindAsync(id);
        await this.users.FindAsync(id);
        users.FindAs<string>(id);
        this.users.FindAs<string>(id);
        servicePool[0].Find(id);
        lazyUsers.Value.Find(id);
        lazyUsers.Value.ExternalProfile.Load(id);
        maybeUsers?.ExternalProfile.Load(id);
        maybeUsers!.ExternalProfile.Load(id);
        servicePool[0].ExternalProfile.Load(id);
        listUsers[0].ExternalProfile.Load(id);
        usersById[id].ExternalProfile.Load(id);
        this.users.Profile.Load(id);
        this.LocalTag(id);
        base.BaseTag(id);
        return ClampName(id);
    }
}
"#;
        let symbols = extract_symbols(source, Language::CSharp, "AuthController.cs").unwrap();
        let calls = extract_calls(source, Language::CSharp, "AuthController.cs", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();
        assert!(callees.contains(&"Audit.Record"));
        assert!(callees.contains(&"App.Support.AuditLog.Record"));
        assert!(callees.contains(&"App.Support.MathUtil.ClampName"));
        assert!(callees.contains(&"UserService.Find"));
        assert!(callees.contains(&"UserService.ExternalProfile.Load"));
        assert!(callees.contains(&"App.Services.UserService.ExternalProfile.Load"));
        assert!(
            callees
                .iter()
                .filter(|callee| **callee == "UserService.Find")
                .count()
                >= 8
        );
        assert!(
            callees
                .iter()
                .filter(|callee| **callee == "UserService.ExternalProfile.Load")
                .count()
                >= 3
        );
        assert!(
            callees
                .iter()
                .filter(|callee| **callee == "App.Services.UserService.ExternalProfile.Load")
                .count()
                >= 3
        );
        assert!(callees.contains(&"users.Find"));
        assert!(callees.contains(&"LocalTag"));
        assert!(callees.contains(&"base.BaseTag"));
        assert!(callees.contains(&"ClampName"));
        assert!(callees.contains(&"users.Profile.Load"));
        assert_eq!(
            callees
                .iter()
                .filter(|callee| **callee == "users.Find")
                .count(),
            5
        );
        assert_eq!(
            callees
                .iter()
                .filter(|callee| **callee == "users.FindAsync")
                .count(),
            2
        );
        assert_eq!(
            callees
                .iter()
                .filter(|callee| **callee == "users.FindAs")
                .count(),
            2
        );
        assert!(callees.contains(&"servicePool.Find"));
        assert!(callees.contains(&"lazyUsers.Value.Find"));
        assert!(callees.contains(&"lazyUsers.Value.ExternalProfile.Load"));
        assert_eq!(
            callees
                .iter()
                .filter(|callee| **callee == "maybeUsers.ExternalProfile.Load")
                .count(),
            2
        );
        assert!(callees.contains(&"servicePool.ExternalProfile.Load"));
        assert!(callees.contains(&"listUsers.ExternalProfile.Load"));
        assert!(callees.contains(&"usersById.ExternalProfile.Load"));
    }

    #[test]
    fn skips_csharp_nested_temporary_wrapper_calls() {
        let source = r#"
public class AuthController {
    public string Login(string id) {
        var nestedMappedUser = new Lazy<Dictionary<string, UserService>>(() => usersById).Value[id].Find(id);
        var nestedListUser = new Task<List<UserService>>(() => listUsers).Result[0].Find(id);
        var nestedMappedExternalProfile = new Lazy<Dictionary<string, UserService>>(() => usersById).Value[id].ExternalProfile.Load(id);
        return nestedMappedUser + nestedListUser + nestedMappedExternalProfile;
    }
}
"#;
        let symbols = extract_symbols(source, Language::CSharp, "AuthController.cs").unwrap();
        let calls = extract_calls(source, Language::CSharp, "AuthController.cs", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();

        assert!(
            callees
                .iter()
                .all(|callee| !callee.starts_with("UserService."))
        );
        assert!(
            callees
                .iter()
                .all(|callee| !callee.starts_with("App.Services.UserService."))
        );
        assert!(!callees.contains(&"Find"));
        assert!(!callees.contains(&"Load"));
    }

    #[test]
    fn skips_csharp_conditional_access_outer_invocations() {
        let source = r#"
public class AuthController {
    public string Login(string id) {
        return ClampName(users.Find(id) + users?.Find(id) + users!.Find(id) + this.users?.Find(id) + this.users!.Find(id));
    }

    public string Profile(string id) {
        return ClampName(maybeUsers?.ExternalProfile.Load(id) + maybeUsers!.ExternalProfile.Load(id) + users?.FormatForDisplay(id));
    }
}
"#;
        let symbols = extract_symbols(source, Language::CSharp, "AuthController.cs").unwrap();
        let calls = extract_calls(source, Language::CSharp, "AuthController.cs", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();

        assert!(callees.contains(&"ClampName"));
        assert!(callees.contains(&"users.Find"));
        assert!(
            callees
                .iter()
                .all(|callee| !callee.contains("users.Find.users.Find"))
        );
        assert!(callees.contains(&"maybeUsers.ExternalProfile.Load"));
        assert!(
            callees
                .iter()
                .all(|callee| !callee.contains("ClampName.users"))
        );
        assert!(
            callees
                .iter()
                .all(|callee| !callee.contains("ClampName.maybeUsers"))
        );
        assert!(callees.iter().all(|callee| {
            !callee.contains("maybeUsers.ExternalProfile.Load.maybeUsers.ExternalProfile.Load")
        }));
        assert!(
            callees
                .iter()
                .all(|callee| !callee.contains("users.FormatForDisplay.users.FormatForDisplay"))
        );
    }

    #[test]
    fn extracts_php_symbols_dependencies_and_calls() {
        let source = r#"<?php
namespace App\Controller;

use App\Repository\UserRepository;
use App\Support\AuditLog;
use function App\Support\audit_login;

class AuthController extends BaseController implements AuthGuard, Auditable {
    private UserRepository $users;
    public const GUARD = 'web';

    public function __construct(UserRepository $users) {
        $this->users = $users;
    }

    public function login(string $id): bool {
        $user = $this->users->find($id);
        AuditLog::record($id);
        audit_login($id);
        $this->audit($user);
        return true;
    }

    private function audit(User $user): void {}
}

interface UserRepository {
    public function find(string $id): User;
}
"#;
        let symbols = extract_symbols(source, Language::Php, "AuthController.php").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AuthController"));
        assert!(names.contains(&"AuthController.users"));
        assert!(names.contains(&"AuthController.GUARD"));
        assert!(names.contains(&"AuthController.__construct"));
        assert!(names.contains(&"AuthController.login"));
        assert!(names.contains(&"AuthController.audit"));
        assert!(names.contains(&"UserRepository"));
        assert!(names.contains(&"UserRepository.find"));

        let deps = extract_dependencies(source, Language::Php, "AuthController.php").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"App\\Repository\\UserRepository"));
        assert!(targets.contains(&"App\\Support\\AuditLog"));
        assert!(targets.contains(&"App\\Support\\audit_login"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "BaseController"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthController")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "AuthGuard"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthController")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "Auditable"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthController")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));

        let calls = extract_calls(source, Language::Php, "AuthController.php", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "AuthController.login" && call.callee == "audit")
        );
        assert!(calls.iter().any(|call| {
            call.caller == "AuthController.login" && call.callee == "AuditLog.record"
        }));
        assert!(
            calls.iter().any(|call| {
                call.caller == "AuthController.login" && call.callee == "audit_login"
            })
        );
    }

    #[test]
    fn builds_php_use_candidate_paths() {
        assert_eq!(
            php_use_candidate_paths("App\\Repository\\UserRepository"),
            vec![
                PathBuf::from("App/Repository/UserRepository.php"),
                PathBuf::from("Repository/UserRepository.php"),
            ]
        );
        assert_eq!(
            php_use_candidate_paths("\\Vendor\\Package\\Client"),
            vec![PathBuf::from("Vendor/Package/Client.php")]
        );
    }

    #[test]
    fn parses_php_use_entries_with_aliases() {
        assert_eq!(
            php_use_entries("use App\\Support\\AuditLog as Audit;"),
            vec![(
                "App\\Support\\AuditLog".to_string(),
                Some("Audit".to_string()),
                Some("*".to_string())
            )]
        );
        assert_eq!(
            php_use_entries("use function App\\Support\\audit_login;"),
            vec![(
                "App\\Support\\audit_login".to_string(),
                Some("audit_login".to_string()),
                Some("audit_login".to_string())
            )]
        );
        assert_eq!(
            php_use_entries("use App\\Support\\{AuditLog, Metrics as MetricsAlias};"),
            vec![
                (
                    "App\\Support\\AuditLog".to_string(),
                    Some("AuditLog".to_string()),
                    Some("*".to_string())
                ),
                (
                    "App\\Support\\Metrics".to_string(),
                    Some("MetricsAlias".to_string()),
                    Some("*".to_string())
                )
            ]
        );
        assert_eq!(
            php_use_entries("use function App\\Support\\{audit_login, audit_event as event};"),
            vec![
                (
                    "App\\Support\\audit_login".to_string(),
                    Some("audit_login".to_string()),
                    Some("audit_login".to_string())
                ),
                (
                    "App\\Support\\audit_event".to_string(),
                    Some("event".to_string()),
                    Some("audit_event".to_string())
                )
            ]
        );
    }

    #[test]
    fn parses_ruby_require_relative_aliases() {
        assert_eq!(
            ruby_require_alias("support/audit_log", "require_relative"),
            (Some("AuditLog".to_string()), Some("*".to_string()))
        );
        assert_eq!(
            ruby_require_alias("../support/audit.rb", "require_relative"),
            (Some("Audit".to_string()), Some("*".to_string()))
        );
        assert_eq!(ruby_require_alias("json", "require"), (None, None));
    }

    #[test]
    fn normalizes_ruby_member_calls() {
        assert_eq!(
            normalize_callee("Audit.record", Language::Ruby),
            Some("Audit.record".to_string())
        );
        assert_eq!(
            normalize_callee("audit(user)", Language::Ruby),
            Some("audit".to_string())
        );
    }

    #[test]
    fn extracts_ruby_symbols_dependencies_and_calls() {
        let source = r#"
require "json"
require_relative "support/audit"

module Example
  class AuthService < BaseService
    TOKEN = "web"

    def initialize(repository)
      @repository = repository
    end

    def login(id)
      user = @repository.find(id)
      Audit.record(user)
      audit(user)
      true
    end

    def self.build(repository)
      new(repository)
    end

    private

    def audit(user)
    end
  end
end
"#;
        let symbols = extract_symbols(source, Language::Ruby, "auth_service.rb").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"Example"));
        assert!(names.contains(&"Example.AuthService"));
        assert!(names.contains(&"Example.AuthService.TOKEN"));
        assert!(names.contains(&"Example.AuthService.initialize"));
        assert!(names.contains(&"Example.AuthService.login"));
        assert!(names.contains(&"Example.AuthService.build"));
        assert!(names.contains(&"Example.AuthService.audit"));

        let deps = extract_dependencies(source, Language::Ruby, "auth_service.rb").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"json"));
        assert!(targets.contains(&"support/audit"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "BaseService"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));

        let calls = extract_calls(source, Language::Ruby, "auth_service.rb", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "Example.AuthService.login" && call.callee == "audit")
        );
        assert!(calls.iter().any(|call| {
            call.caller == "Example.AuthService.login" && call.callee == "Audit.record"
        }));
    }

    #[test]
    fn extracts_java_symbols_dependencies_and_calls() {
        let source = r#"
package com.example.auth;

import java.util.List;
import static java.util.Collections.emptyList;

public class AuthService extends BaseAuthService implements UserRepository, Auditable<User, Role> {
    private String token;

    public AuthService() {}

    public boolean login(User user) {
        audit(user);
        return true;
    }

    private void audit(User user) {}
}

interface UserRepository {
    User find(String id);
}

interface Auditable<T, R> {}

class BaseAuthService {}
"#;
        let symbols = extract_symbols(source, Language::Java, "AuthService.java").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"AuthService"));
        assert!(names.contains(&"AuthService.token"));
        assert!(names.contains(&"AuthService.AuthService"));
        assert!(names.contains(&"AuthService.login"));
        assert!(names.contains(&"AuthService.audit"));
        assert!(names.contains(&"UserRepository"));
        assert!(names.contains(&"UserRepository.find"));

        let deps = extract_dependencies(source, Language::Java, "AuthService.java").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"com.example.auth"));
        assert!(targets.contains(&"java.util.List"));
        assert!(targets.contains(&"java.util.Collections.emptyList"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "BaseAuthService"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("extends")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "UserRepository"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "Auditable"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("AuthService")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));
        assert!(deps.iter().all(|dependency| dependency.target != "Role>"));

        let calls = extract_calls(source, Language::Java, "AuthService.java", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| call.caller == "AuthService.login" && call.callee == "audit")
        );
    }

    #[test]
    fn parses_java_import_aliases() {
        assert_eq!(
            java_import_alias("com.example.auth.AuthService", "import"),
            (Some("AuthService".to_string()), Some("*".to_string()))
        );
        assert_eq!(
            java_import_alias("com.example.util.Names.defaultName", "static_import"),
            (
                Some("defaultName".to_string()),
                Some("defaultName".to_string())
            )
        );
    }

    #[test]
    fn normalizes_java_qualified_calls() {
        let source = r#"
public class App {
    public String run() {
        return AuthService.login(defaultName());
    }
}
"#;
        let symbols = extract_symbols(source, Language::Java, "App.java").unwrap();
        let calls = extract_calls(source, Language::Java, "App.java", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();
        assert!(callees.contains(&"AuthService.login"));
        assert!(callees.contains(&"defaultName"));
    }

    #[test]
    fn builds_java_import_candidate_paths() {
        assert_eq!(
            java_import_candidate_paths("com.example.auth.AuthService", false),
            vec![PathBuf::from("com/example/auth/AuthService.java")]
        );
        assert_eq!(
            java_import_candidate_paths("com.example.util.Names.defaultName", true),
            vec![
                PathBuf::from("com/example/util/Names/defaultName.java"),
                PathBuf::from("com/example/util/Names.java"),
                PathBuf::from("com/example/util.java"),
                PathBuf::from("com/example.java"),
                PathBuf::from("com.java"),
            ]
        );
    }

    #[test]
    fn extracts_rust_symbols() {
        let source = r#"
struct Store {}

trait Repository<T> {}

impl Store {
    fn open() {}
}

impl<T> Repository<T> for Store {}

fn helper() {}
"#;
        let symbols = extract_symbols(source, Language::Rust, "storage.rs").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"Store"));
        assert!(names.contains(&"Repository"));
        assert!(names.contains(&"open"));
        assert!(names.contains(&"helper"));

        let deps = extract_dependencies(source, Language::Rust, "storage.rs").unwrap();
        assert!(deps.iter().any(|dependency| {
            dependency.target == "Repository"
                && dependency.kind == "base_type"
                && dependency.local_alias.as_deref() == Some("Store")
                && dependency.imported_symbol.as_deref() == Some("implements")
        }));
        assert_eq!(
            deps.iter()
                .filter(|dependency| dependency.kind == "base_type")
                .count(),
            1
        );
    }

    #[test]
    fn builds_rust_use_path_candidates() {
        assert_eq!(
            rust_use_path_candidates(PathBuf::from("src"), "support::audit::record"),
            vec![
                PathBuf::from("src/support/audit/record.rs"),
                PathBuf::from("src/support/audit/record/mod.rs"),
                PathBuf::from("src/support/audit.rs"),
                PathBuf::from("src/support/audit/mod.rs"),
                PathBuf::from("src/support.rs"),
                PathBuf::from("src/support/mod.rs"),
            ]
        );
        assert_eq!(
            rust_crate_source_root("crates/app/src/lib.rs"),
            PathBuf::from("crates/app/src")
        );
        assert_eq!(
            rust_super_module_dir("src/controllers/auth.rs"),
            PathBuf::from("src/controllers")
        );
    }

    #[test]
    fn parses_rust_use_aliases() {
        assert_eq!(
            rust_use_targets("use crate::support::audit as audit_log;"),
            vec!["crate::support::audit"]
        );
        assert_eq!(
            rust_use_alias("crate::support::audit", Some("audit_log")),
            (Some("audit_log".to_string()), Some("audit".to_string()))
        );
        assert_eq!(
            rust_use_alias("super::support::helper", None),
            (Some("helper".to_string()), Some("*".to_string()))
        );
    }

    #[test]
    fn normalizes_rust_scoped_calls() {
        let source = r#"
pub fn run() {
    audit::record("root");
    helper("id");
}
"#;
        let symbols = extract_symbols(source, Language::Rust, "lib.rs").unwrap();
        let calls = extract_calls(source, Language::Rust, "lib.rs", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();
        assert!(callees.contains(&"audit.record"));
        assert!(callees.contains(&"helper"));
    }

    #[test]
    fn parses_python_from_import_member_targets() {
        assert_eq!(
            python_import_targets("from .support import audit, logger as log"),
            vec![
                ".support".to_string(),
                ".support.audit".to_string(),
                ".support.logger".to_string(),
            ]
        );
        assert_eq!(
            python_import_targets("from . import support"),
            vec![".".to_string(), ".support".to_string()]
        );
        assert_eq!(
            python_import_targets("from app.shared import ping as shared_ping"),
            vec!["app.shared".to_string(), "app.shared.ping".to_string()]
        );
        assert_eq!(
            python_import_targets(
                "from app.shared import (\n  ping as shared_ping,\n  tools as shared_tools,\n)"
            ),
            vec![
                "app.shared".to_string(),
                "app.shared.ping".to_string(),
                "app.shared.tools".to_string(),
            ]
        );
        assert_eq!(
            python_target_candidates(PathBuf::from("app/controllers/support/audit")),
            vec![
                PathBuf::from("app/controllers/support/audit"),
                PathBuf::from("app/controllers/support"),
                PathBuf::from("app/controllers"),
                PathBuf::from("app"),
            ]
        );
        assert_eq!(
            python_target_candidates(PathBuf::from("app/shared/ping")),
            vec![
                PathBuf::from("app/shared/ping"),
                PathBuf::from("app/shared"),
                PathBuf::from("app"),
            ]
        );
    }

    #[test]
    fn extracts_dependencies() {
        let ts = r#"
import { readFile as loadFile } from "node:fs";
import loadDefault from "node:fs";
import * as pathApi from "node:path";
const auth = require("./auth");
const { render: draw } = require("./ui");
const uiModule = require("./ui");
const loadedUi = await import("./ui");
const computedUi = require("./" + "ui");
const modalPath = "./modal";
const modalModule = require(modalPath);
const modalLoaded = await import(modalPath);
import(modalPath).then((thenModal) => thenModal.render());
require(modalPath).render();
import("./ui").then((thenUi) => thenUi.render());
require("./" + "ui").render();
export { render as relayRender, default as relayDefault } from "./ui";
export * from "./all-ui";
export * as uiApi from "./ui";
"#;
        let deps = extract_dependencies(ts, Language::TypeScript, "src/index.ts").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"node:fs"));
        assert!(targets.contains(&"./auth"));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "node:fs"
                && dependency.local_alias.as_deref() == Some("loadFile")
                && dependency.imported_symbol.as_deref() == Some("readFile")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "node:fs"
                && dependency.local_alias.as_deref() == Some("loadDefault")
                && dependency.imported_symbol.as_deref() == Some("default")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.local_alias.as_deref() == Some("draw")
                && dependency.imported_symbol.as_deref() == Some("render")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "node:path"
                && dependency.local_alias.as_deref() == Some("pathApi")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.local_alias.as_deref() == Some("uiModule")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.local_alias.as_deref() == Some("computedUi")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("loadedUi")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("thenUi")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "export_alias"
                && dependency.local_alias.as_deref() == Some("relayRender")
                && dependency.imported_symbol.as_deref() == Some("render")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "export_alias"
                && dependency.local_alias.as_deref() == Some("relayDefault")
                && dependency.imported_symbol.as_deref() == Some("default")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./all-ui"
                && dependency.kind == "export_namespace"
                && dependency.local_alias.is_none()
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "export_namespace"
                && dependency.local_alias.as_deref() == Some("uiApi")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./modal"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("modalModule")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./modal"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("modalLoaded")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./modal"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("thenModal")
                && dependency.imported_symbol.as_deref() == Some("*")
        }));
        assert!(
            deps.iter().any(|dependency| {
                dependency.target == "./modal" && dependency.kind == "import"
            })
        );

        let chained_bindings = r#"
const root = "./";
const path = root + "modal";
const chainedModule = require(path);
"#;
        let deps =
            extract_dependencies(chained_bindings, Language::TypeScript, "src/chained.ts").unwrap();
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./modal"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("chainedModule")
        }));

        let interpolated_template = r#"
const templateRoot = "./";
const templatePath = `${templateRoot}ui`;
const templateModule = require(templatePath);
"#;
        let deps = extract_dependencies(
            interpolated_template,
            Language::TypeScript,
            "src/template.ts",
        )
        .unwrap();
        assert!(deps.iter().any(|dependency| {
            dependency.target == "./ui"
                && dependency.kind == "import_namespace"
                && dependency.local_alias.as_deref() == Some("templateModule")
        }));

        let py = "from app.auth import service\nimport os, sys\n";
        let deps = extract_dependencies(py, Language::Python, "app/main.py").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"app.auth"));
        assert!(targets.contains(&"os"));
        assert!(targets.contains(&"sys"));
    }

    #[test]
    fn resolves_relative_dependencies() {
        let dir = tempfile::TempDir::new().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("main.ts"),
            "import { AuthService } from './auth';\n",
        )
        .unwrap();
        std::fs::write(src.join("auth.ts"), "export class AuthService {}\n").unwrap();

        let report = index_project(dir.path(), false).unwrap();
        assert_eq!(report.errors.len(), 0);

        let store = Store::open(dir.path()).unwrap();
        let graph = store
            .dependency_graph(dir.path(), 10, 0, &[], &[], &[])
            .unwrap();
        assert!(graph.dependencies.iter().any(|dependency| {
            dependency.source_file == "src/main.ts"
                && dependency.target == "./auth"
                && dependency.resolved_file.as_deref() == Some("src/auth.ts")
        }));
    }

    #[test]
    fn extracts_same_file_calls() {
        let source = r#"
class AuthService:
    def login(self):
        return helper()

def helper():
    return "ok"
"#;
        let symbols = extract_symbols(source, Language::Python, "auth.py").unwrap();
        let calls = extract_calls(source, Language::Python, "auth.py", &symbols);
        assert!(
            calls
                .iter()
                .any(|call| { call.caller == "AuthService.login" && call.callee == "helper" })
        );
        let helper_call = calls
            .iter()
            .find(|call| call.caller == "AuthService.login" && call.callee == "helper")
            .unwrap();
        assert_eq!(helper_call.callee_file.as_deref(), Some("auth.py"));
        assert!(helper_call.confidence >= 0.72);
    }

    #[test]
    fn normalizes_python_member_calls() {
        let source = r#"
class AuthController:
    def login(self, user_id):
        audit.record(user_id)
        return service.load(user_id)
"#;
        let symbols = extract_symbols(source, Language::Python, "auth.py").unwrap();
        let calls = extract_calls(source, Language::Python, "auth.py", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();
        assert!(callees.contains(&"audit.record"));
        assert!(callees.contains(&"service.load"));
    }

    #[test]
    fn normalizes_javascript_member_and_computed_calls() {
        let source = r#"
function register(app, method, handler) {
  app.get("/ok", handler);
  app?.put("/ok", handler);
  app.delete?.("/ok", handler);
  app["post"]("/ok", handler);
  app[method]("/ok", handler);
  router.route("/ok").patch(handler);
  module.exports.create();
  require("./ui").render();
  helper();
}
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "routes.js").unwrap();
        let calls = extract_calls(source, Language::JavaScript, "routes.js", &symbols);
        let callees = calls
            .iter()
            .map(|call| call.callee.as_str())
            .collect::<Vec<_>>();

        assert!(callees.contains(&"app.get"));
        assert!(callees.contains(&"app.put"));
        assert!(callees.contains(&"app.delete"));
        assert!(callees.contains(&"app.post"));
        assert!(callees.contains(&"app.<dynamic>"));
        assert!(callees.contains(&"router.route.patch"));
        assert!(callees.contains(&"module.exports.create"));
        assert!(callees.contains(&"require.render"));
        assert!(callees.contains(&"helper"));
    }

    #[test]
    fn attributes_javascript_callback_callers() {
        let source = r#"
describe("routes", function () {
  it("registers", function () {
    app.route("/ok").get(function (req, res) {
      res.send("ok");
    });
  });
});
"#;
        let symbols = extract_symbols(source, Language::JavaScript, "routes.test.js").unwrap();
        let calls = extract_calls(source, Language::JavaScript, "routes.test.js", &symbols);

        assert!(
            calls
                .iter()
                .any(|call| { call.caller == "describe.<callback>" && call.callee == "it" })
        );
        assert!(
            calls
                .iter()
                .any(|call| { call.caller == "it.<callback>" && call.callee == "app.route.get" })
        );
        assert!(calls.iter().any(|call| {
            call.caller == "app.route.get.<callback>" && call.callee == "res.send"
        }));
    }

    #[test]
    fn skips_unchanged_files_and_removes_deleted_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let source_path = dir.path().join("auth.py");
        std::fs::write(&source_path, "def login():\n    pass\n").unwrap();

        let first = index_project(dir.path(), false).unwrap();
        assert_eq!(first.schema_version, SCHEMA_VERSION);
        assert_eq!(first.index_version, INDEX_VERSION);
        assert_eq!(first.indexed_files, 1);
        assert_eq!(first.changed_files, 1);
        assert_eq!(first.unchanged_files, 0);
        assert!(first.errors.is_empty());

        let second = index_project(dir.path(), false).unwrap();
        assert_eq!(second.indexed_files, 1);
        assert_eq!(second.changed_files, 0);
        assert_eq!(second.unchanged_files, 1);

        std::fs::remove_file(&source_path).unwrap();
        let third = index_project(dir.path(), false).unwrap();
        assert_eq!(third.indexed_files, 0);
        assert_eq!(third.deleted_files, 1);
    }
}
