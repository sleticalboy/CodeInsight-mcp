use std::path::Path;

use tree_sitter::Language as TsLanguage;

use crate::model::Language;

pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("go") => Some(Language::Go),
        Some("js") | Some("mjs") | Some("cjs") => Some(Language::JavaScript),
        Some("py") => Some(Language::Python),
        Some("rs") => Some(Language::Rust),
        Some("ts") => Some(Language::TypeScript),
        Some("tsx") => Some(Language::Tsx),
        _ => None,
    }
}

pub fn tree_sitter_language(language: Language) -> TsLanguage {
    match language {
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
    }
}
