use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser, TreeCursor};

use crate::{
    language::{detect_language, tree_sitter_language},
    model::{
        CallEdge, Dependency, IndexError, Language, ProjectIndexReport, SourceFile, Symbol,
        SymbolKind,
    },
    storage::{INDEX_VERSION, SCHEMA_VERSION, Store},
};

pub fn index_project(root: &Path, force: bool) -> Result<ProjectIndexReport> {
    let started = Instant::now();
    let root = root.canonicalize()?;
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

        let relative_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
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
        resolve_dependencies(&root, &mut dependencies);
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
    let mut dependencies = Vec::new();
    visit_dependency_node(
        tree.root_node(),
        source.as_bytes(),
        language,
        source_file,
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
        && let Some(raw_target) = call_target_text(node, source)
        && let Some(callee) = normalize_callee(&raw_target, language)
    {
        let line = node.start_position().row + 1;
        let caller = caller_for_call_node(node, source, language, symbols, line)
            .unwrap_or_else(|| "<module>".to_string());
        calls.push(CallEdge {
            file: source_file.to_string(),
            caller,
            callee,
            callee_file: None,
            language,
            line,
            column: node.start_position().column + 1,
            confidence: 0.55,
        });
    }

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        visit_call_node(child, source, language, source_file, symbols, calls);
    }
}

fn is_call_node(node: Node<'_>, language: Language) -> bool {
    match language {
        Language::Go
        | Language::Rust
        | Language::JavaScript
        | Language::TypeScript
        | Language::Tsx => node.kind() == "call_expression",
        Language::Python => node.kind() == "call",
    }
}

fn call_target_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("function")
        .or_else(|| node.child_by_field_name("name"))
        .or_else(|| node.child(0))
        .and_then(|child| child.utf8_text(source).ok())
        .map(ToOwned::to_owned)
}

fn normalize_callee(raw: &str, language: Language) -> Option<String> {
    if matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        return normalize_javascript_callee(raw);
    }

    normalize_simple_callee(raw)
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
    dependencies: &mut Vec<Dependency>,
) {
    dependencies.extend(dependencies_from_node(node, source, language, source_file));

    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        visit_dependency_node(child, source, language, source_file, dependencies);
    }
}

fn dependencies_from_node(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match language {
        Language::Python => python_dependencies(node, source, language, source_file),
        Language::Go => go_dependencies(node, source, language, source_file),
        Language::Rust => rust_dependencies(node, source, language, source_file),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_like_dependencies(node, source, language, source_file)
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

fn python_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "import_statement" | "import_from_statement" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "import",
            python_import_targets,
        ),
        _ => Vec::new(),
    }
}

fn go_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "import_declaration" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "import",
            string_literal_targets,
        ),
        _ => Vec::new(),
    }
}

fn rust_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
) -> Vec<Dependency> {
    match node.kind() {
        "use_declaration" => {
            text_dependencies(node, source, language, source_file, "use", rust_use_targets)
        }
        "mod_item" => {
            text_dependencies(node, source, language, source_file, "mod", rust_mod_targets)
        }
        _ => Vec::new(),
    }
}

fn javascript_like_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
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
        "export_statement" | "call_expression" => text_dependencies(
            node,
            source,
            language,
            source_file,
            "import",
            string_literal_targets,
        ),
        "variable_declarator" => {
            javascript_require_alias_dependencies(node, source, language, source_file)
        }
        _ => Vec::new(),
    }
}

fn text_dependencies(
    node: Node<'_>,
    source: &[u8],
    language: Language,
    source_file: &str,
    kind: &str,
    extractor: fn(&str) -> Vec<String>,
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
    let Some(target) = string_literal_targets(value).into_iter().next() else {
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

fn resolve_dependencies(root: &Path, dependencies: &mut [Dependency]) {
    for dependency in dependencies {
        dependency.resolved_file = resolve_dependency(root, dependency);
    }
}

fn resolve_dependency(root: &Path, dependency: &Dependency) -> Option<String> {
    match dependency.language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => resolve_relative_target(
            root,
            &dependency.source_file,
            &dependency.target,
            &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        ),
        Language::Python => resolve_python_target(root, dependency),
        Language::Rust => resolve_rust_target(root, dependency),
        Language::Go => None,
    }
}

fn resolve_python_target(root: &Path, dependency: &Dependency) -> Option<String> {
    if dependency.target.starts_with('.') {
        let relative = dependency.target.replace('.', "/");
        resolve_relative_target(root, &dependency.source_file, &relative, &["py"])
    } else {
        resolve_module_target(root, &dependency.target.replace('.', "/"), &["py"])
    }
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

fn resolve_module_target(root: &Path, target: &str, extensions: &[&str]) -> Option<String> {
    resolve_base(root, PathBuf::from(target), extensions)
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

fn python_import_targets(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("from ") {
        return rest
            .split_whitespace()
            .next()
            .map(|target| vec![target.to_string()])
            .unwrap_or_default();
    }

    trimmed
        .strip_prefix("import ")
        .map(|rest| {
            rest.split(',')
                .filter_map(|part| part.split_whitespace().next())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn rust_use_targets(text: &str) -> Vec<String> {
    text.trim()
        .strip_prefix("use ")
        .map(|target| {
            let cleaned = target
                .trim_end_matches(';')
                .trim()
                .replace("::{", "::")
                .replace(['{', '}'], "");
            vec![compact_whitespace(&cleaned)]
        })
        .unwrap_or_default()
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
        Language::Python => python_symbol(node, source),
        Language::Rust => rust_symbol(node, source),
        Language::Go => go_symbol(node, source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_like_symbol(node, source)
        }
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

fn javascript_like_symbol(node: Node<'_>, source: &[u8]) -> Option<(String, SymbolKind)> {
    match node.kind() {
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
    path.components().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
export class AuthService {
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
    fn extracts_rust_symbols() {
        let source = r#"
struct Store {}

impl Store {
    fn open() {}
}

fn helper() {}
"#;
        let symbols = extract_symbols(source, Language::Rust, "storage.rs").unwrap();
        let names = symbols
            .iter()
            .map(|symbol| symbol.qualified_name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"Store"));
        assert!(names.contains(&"open"));
        assert!(names.contains(&"helper"));
    }

    #[test]
    fn extracts_dependencies() {
        let ts = r#"
import { readFile as loadFile } from "node:fs";
import * as pathApi from "node:path";
const auth = require("./auth");
const { render: draw } = require("./ui");
const uiModule = require("./ui");
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
        let graph = store.dependency_graph(dir.path(), 10).unwrap();
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
