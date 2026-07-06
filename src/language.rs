use std::path::Path;

use tree_sitter::Language as TsLanguage;

use crate::model::Language;

pub fn detect_language(path: &Path) -> Option<Language> {
    match path.extension().and_then(|value| value.to_str()) {
        Some("c") | Some("h") => Some(Language::C),
        Some("cc") | Some("cpp") | Some("cxx") | Some("hh") | Some("hpp") | Some("hxx") => {
            Some(Language::Cpp)
        }
        Some("cs") => Some(Language::CSharp),
        Some("go") => Some(Language::Go),
        Some("java") => Some(Language::Java),
        Some("js") | Some("mjs") | Some("cjs") => Some(Language::JavaScript),
        Some("php") => Some(Language::Php),
        Some("py") => Some(Language::Python),
        Some("rb") => Some(Language::Ruby),
        Some("rs") => Some(Language::Rust),
        Some("ts") => Some(Language::TypeScript),
        Some("tsx") => Some(Language::Tsx),
        _ => None,
    }
}

pub fn tree_sitter_language(language: Language) -> TsLanguage {
    match language {
        Language::C => tree_sitter_c::LANGUAGE.into(),
        Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
    }
}
