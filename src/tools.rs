use std::path::PathBuf;

use anyhow::Result;

use crate::{index, storage::Store};

pub fn index_project(root: PathBuf, force: bool) -> Result<()> {
    let report = index::index_project(&root, force)?;
    print_json(&report)
}

pub fn project_overview(root: PathBuf) -> Result<()> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    let overview = store.overview(&root)?;
    print_json(&overview)
}

pub fn symbol_search(root: PathBuf, query: String, limit: usize) -> Result<()> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    let symbols = store.search_symbols(&query, limit)?;
    print_json(&symbols)
}

pub fn file_outline(path: PathBuf) -> Result<()> {
    let symbols = index::outline_file(&path)?;
    print_json(&symbols)
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
