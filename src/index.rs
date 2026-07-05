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
        let dependencies = match extract_dependencies(&source, language, &relative_path) {
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
        && let Some(callee) = normalize_callee(&raw_target)
    {
        let line = node.start_position().row + 1;
        let caller = caller_for_line(symbols, line).unwrap_or_else(|| "<module>".to_string());
        calls.push(CallEdge {
            file: source_file.to_string(),
            caller,
            callee,
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

fn normalize_callee(raw: &str) -> Option<String> {
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
        "import_statement" | "export_statement" | "call_expression" => text_dependencies(
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
            target,
            kind: kind.to_string(),
            language,
            line: node.start_position().row + 1,
        })
        .collect()
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
        "class_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Class))
        }
        "method_definition" | "public_field_definition" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Method))
        }
        "interface_declaration" => {
            child_text(node, "name", source).map(|name| (name, SymbolKind::Interface))
        }
        "lexical_declaration" | "variable_declaration" => {
            find_js_variable_name(node, source).map(|name| (name, SymbolKind::Variable))
        }
        _ => None,
    }
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

fn find_js_variable_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node_children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }
        if let Some(name) = child_text(child, "name", source) {
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
import { readFile } from "node:fs";
const auth = require("./auth");
"#;
        let deps = extract_dependencies(ts, Language::TypeScript, "src/index.ts").unwrap();
        let targets = deps
            .iter()
            .map(|dependency| dependency.target.as_str())
            .collect::<Vec<_>>();
        assert!(targets.contains(&"node:fs"));
        assert!(targets.contains(&"./auth"));

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
