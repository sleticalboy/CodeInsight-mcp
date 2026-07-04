use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::model::{
    DirectoryStat, Language, LanguageStat, ProjectOverview, SourceFile, Symbol, SymbolKind,
};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let dir = cache_dir(root);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create cache directory {}", dir.display()))?;
        let db_path = dir.join("index.db");
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn reset(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("delete from symbols", [])?;
        tx.execute("delete from files", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_file(&mut self, file: &SourceFile) -> Result<i64> {
        self.conn.execute(
            "insert into files (path, language, hash, line_count)
             values (?1, ?2, ?3, ?4)
             on conflict(path) do update set
               language = excluded.language,
               hash = excluded.hash,
               line_count = excluded.line_count",
            params![
                file.relative_path,
                file.language.as_str(),
                file.hash,
                file.line_count as i64
            ],
        )?;
        Ok(self.conn.query_row(
            "select id from files where path = ?1",
            params![file.relative_path],
            |row| row.get(0),
        )?)
    }

    pub fn replace_symbols(&mut self, file_id: i64, symbols: &[Symbol]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("delete from symbols where file_id = ?1", params![file_id])?;
        {
            let mut stmt = tx.prepare(
                "insert into symbols
                 (file_id, name, qualified_name, kind, language, start_line, end_line)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for symbol in symbols {
                stmt.execute(params![
                    file_id,
                    symbol.name,
                    symbol.qualified_name,
                    symbol_kind(symbol.kind.clone()),
                    symbol.language.as_str(),
                    symbol.start_line as i64,
                    symbol.end_line as i64
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn overview(&self, root: &Path) -> Result<ProjectOverview> {
        let indexed_files = self
            .conn
            .query_row("select count(*) from files", [], |row| row.get::<_, i64>(0))?
            as usize;
        let symbols = self
            .conn
            .query_row("select count(*) from symbols", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;

        let mut lang_stmt = self.conn.prepare(
            "select language, count(*), coalesce(sum(line_count), 0)
             from files group by language order by count(*) desc",
        )?;
        let languages = lang_stmt
            .query_map([], |row| {
                Ok(LanguageStat {
                    language: row.get(0)?,
                    files: row.get::<_, i64>(1)? as usize,
                    lines: row.get::<_, i64>(2)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut dir_stmt = self.conn.prepare(
            "select
                case
                  when instr(path, '/') = 0 then '.'
                  else substr(path, 1, instr(path, '/') - 1)
                end as directory,
                count(*)
             from files
             group by directory
             order by count(*) desc
             limit 12",
        )?;
        let top_directories = dir_stmt
            .query_map([], |row| {
                Ok(DirectoryStat {
                    directory: row.get(0)?,
                    files: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(ProjectOverview {
            root: root.display().to_string(),
            indexed_files,
            symbols,
            languages,
            top_directories,
        })
    }

    pub fn search_symbols(&self, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        let like = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "select s.name, s.qualified_name, s.kind, s.language, f.path, s.start_line, s.end_line
             from symbols s
             join files f on f.id = s.file_id
             where s.name like ?1 or s.qualified_name like ?1
             order by
               case when s.name = ?2 then 0 when s.name like ?3 then 1 else 2 end,
               length(s.qualified_name)
             limit ?4",
        )?;
        let prefix = format!("{query}%");
        let rows = stmt.query_map(params![like, query, prefix, limit as i64], |row| {
            let language: String = row.get(3)?;
            let kind: String = row.get(2)?;
            Ok(Symbol {
                name: row.get(0)?,
                qualified_name: row.get(1)?,
                kind: parse_symbol_kind(&kind),
                language: parse_language(&language),
                file: row.get(4)?,
                start_line: row.get::<_, i64>(5)? as usize,
                end_line: row.get::<_, i64>(6)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            create table if not exists files (
                id integer primary key autoincrement,
                path text not null unique,
                language text not null,
                hash text not null,
                line_count integer not null
            );

            create table if not exists symbols (
                id integer primary key autoincrement,
                file_id integer not null references files(id) on delete cascade,
                name text not null,
                qualified_name text not null,
                kind text not null,
                language text not null,
                start_line integer not null,
                end_line integer not null
            );

            create index if not exists idx_symbols_name on symbols(name);
            create index if not exists idx_symbols_qualified_name on symbols(qualified_name);
            ",
        )?;
        Ok(())
    }
}

pub fn cache_dir(root: &Path) -> PathBuf {
    root.join(".codeinsight")
}

fn symbol_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Class => "class",
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Interface => "interface",
        SymbolKind::Struct => "struct",
        SymbolKind::Variable => "variable",
        SymbolKind::Constant => "constant",
    }
}

fn parse_symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "class" => SymbolKind::Class,
        "method" => SymbolKind::Method,
        "interface" => SymbolKind::Interface,
        "struct" => SymbolKind::Struct,
        "variable" => SymbolKind::Variable,
        "constant" => SymbolKind::Constant,
        _ => SymbolKind::Function,
    }
}

fn parse_language(language: &str) -> Language {
    match language {
        "go" => Language::Go,
        "python" => Language::Python,
        "rust" => Language::Rust,
        "typescript" => Language::TypeScript,
        "tsx" => Language::Tsx,
        _ => Language::JavaScript,
    }
}
