use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rusqlite::{
    Connection, OptionalExtension, Row, params, params_from_iter, types::Value as SqlValue,
};
use serde_json::json;

use crate::model::{
    CallEdge, CallSummary, Dependency, DependencyGraph, DependencySourceStat, DependencySummary,
    DependencyTargetStat, DirectoryStat, DirectorySummary, EntryPointCandidate, IndexStatus,
    Language, LanguageStat, ProjectOverview, RecommendedToolCall, SemanticChunk,
    SemanticChunkChange, SemanticChunkInput, SemanticChunkWriteStats, SemanticEmbeddingInput,
    SemanticEmbeddingMatch, SourceFile, Symbol, SymbolKind, SymbolKindStat,
};

pub const SCHEMA_VERSION: i64 = 22;
pub const INDEX_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIndexMetadata {
    pub hash: String,
    pub size: Option<i64>,
    pub modified_ns: Option<i64>,
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
        store.ensure_schema_version()?;
        Ok(store)
    }

    pub fn reset(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("delete from semantic_embeddings", [])?;
        tx.execute("delete from semantic_chunks", [])?;
        tx.execute("delete from calls", [])?;
        tx.execute("delete from dependencies", [])?;
        tx.execute("delete from symbols", [])?;
        tx.execute("delete from files", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn mark_indexed(&self, resolution_fingerprint: &str) -> Result<()> {
        self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
        self.set_meta("index_version", INDEX_VERSION)?;
        self.set_meta("resolution_fingerprint", resolution_fingerprint)?;
        self.set_meta("last_indexed_at", &unix_timestamp().to_string())?;
        Ok(())
    }

    pub fn resolution_fingerprint(&self) -> Result<Option<String>> {
        self.get_meta("resolution_fingerprint")
    }

    pub fn replace_dependencies(
        &mut self,
        file_id: i64,
        dependencies: &[Dependency],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "delete from dependencies where source_file_id = ?1",
            params![file_id],
        )?;
        {
            let mut stmt = tx.prepare(
                "insert into dependencies
                 (source_file_id, target, resolved_file, local_alias, imported_symbol, kind, language, line)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for dependency in dependencies {
                stmt.execute(params![
                    file_id,
                    dependency.target,
                    dependency.resolved_file,
                    dependency.local_alias,
                    dependency.imported_symbol,
                    dependency.kind,
                    dependency.language.as_str(),
                    dependency.line as i64
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_dependencies_for_file(
        &mut self,
        relative_path: &str,
        dependencies: &[Dependency],
    ) -> Result<()> {
        let file_id = self
            .conn
            .query_row(
                "select id from files where path = ?1",
                params![relative_path],
                |row| row.get(0),
            )
            .optional()?
            .with_context(|| format!("indexed file not found: {relative_path}"))?;
        self.replace_dependencies(file_id, dependencies)
    }

    pub fn indexed_dependencies(&self) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol,
                    d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             order by f.path, d.line",
        )?;
        let rows = stmt.query_map([], |row| {
            let language: String = row.get(6)?;
            Ok(Dependency {
                source_file: row.get(0)?,
                target: row.get(1)?,
                resolved_file: row.get(2)?,
                local_alias: row.get(3)?,
                imported_symbol: row.get(4)?,
                kind: row.get(5)?,
                language: parse_language(&language),
                line: row.get::<_, i64>(7)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn replace_calls(&mut self, file_id: i64, calls: &[CallEdge]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "delete from calls where source_file_id = ?1",
            params![file_id],
        )?;
        {
            let mut stmt = tx.prepare(
                "insert into calls
                 (source_file_id, caller, callee, callee_file, language, line, column, confidence)
                 values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for call in calls {
                stmt.execute(params![
                    file_id,
                    call.caller,
                    call.callee,
                    call.callee_file,
                    call.language.as_str(),
                    call.line as i64,
                    call.column as i64,
                    call.confidence
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub fn upsert_file(&mut self, file: &SourceFile) -> Result<i64> {
        self.upsert_file_with_metadata(file, None, None)
    }

    pub fn upsert_file_with_metadata(
        &mut self,
        file: &SourceFile,
        size: Option<i64>,
        modified_ns: Option<i64>,
    ) -> Result<i64> {
        self.conn.execute(
            "insert into files (path, language, hash, line_count, size, modified_ns)
             values (?1, ?2, ?3, ?4, ?5, ?6)
             on conflict(path) do update set
               language = excluded.language,
               hash = excluded.hash,
               line_count = excluded.line_count,
               size = excluded.size,
               modified_ns = excluded.modified_ns",
            params![
                file.relative_path,
                file.language.as_str(),
                file.hash,
                file.line_count as i64,
                size,
                modified_ns
            ],
        )?;
        Ok(self.conn.query_row(
            "select id from files where path = ?1",
            params![file.relative_path],
            |row| row.get(0),
        )?)
    }

    pub fn file_index_metadata(&self, relative_path: &str) -> Result<Option<FileIndexMetadata>> {
        let mut stmt = self
            .conn
            .prepare("select hash, size, modified_ns from files where path = ?1")?;
        let mut rows = stmt.query(params![relative_path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(FileIndexMetadata {
                hash: row.get(0)?,
                size: row.get(1)?,
                modified_ns: row.get(2)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn count_files(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("select count(*) from files", [], |row| row.get::<_, i64>(0))?
            as usize)
    }

    pub fn count_symbols(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("select count(*) from symbols", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn delete_files_not_in(&mut self, relative_paths: &[String]) -> Result<usize> {
        let existing = self.indexed_files()?;
        let current = relative_paths
            .iter()
            .collect::<std::collections::HashSet<_>>();
        let stale = existing
            .into_iter()
            .filter(|path| !current.contains(path))
            .collect::<Vec<_>>();
        let deleted = stale.len();

        let tx = self.conn.transaction()?;
        for path in stale {
            tx.execute(
                "delete from semantic_embeddings
                 where chunk_id in (
                   select sc.id from semantic_chunks sc
                   join files f on f.id = sc.file_id
                   where f.path = ?1
                 )",
                params![path],
            )?;
            tx.execute(
                "delete from semantic_chunks where file_id in (select id from files where path = ?1)",
                params![path],
            )?;
            tx.execute(
                "delete from calls where source_file_id in (select id from files where path = ?1)",
                params![path],
            )?;
            tx.execute(
                "delete from dependencies where source_file_id in (select id from files where path = ?1)",
                params![path],
            )?;
            tx.execute(
                "delete from symbols where file_id in (select id from files where path = ?1)",
                params![path],
            )?;
            tx.execute("delete from files where path = ?1", params![path])?;
        }
        tx.commit()?;

        Ok(deleted)
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

    pub fn replace_semantic_chunks(
        &mut self,
        chunks: &[SemanticChunkInput],
        explain_changes: bool,
    ) -> Result<SemanticChunkWriteStats> {
        let tx = self.conn.transaction()?;
        let updated_at = unix_timestamp();

        tx.execute("drop table if exists temp.desired_semantic_chunks", [])?;
        tx.execute(
            "create temp table desired_semantic_chunks (
                file_id integer not null,
                start_line integer not null,
                end_line integer not null,
                content_hash text not null,
                token_estimate integer not null,
                text text not null,
                primary key (file_id, start_line, end_line)
            )",
            [],
        )?;
        {
            let mut stmt = tx.prepare(
                "insert or replace into temp.desired_semantic_chunks
                 (file_id, start_line, end_line, content_hash, token_estimate, text)
                 select id, ?2, ?3, ?4, ?5, ?6
                 from files
                 where path = ?1",
            )?;
            for chunk in chunks {
                stmt.execute(params![
                    chunk.file,
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    chunk.content_hash,
                    chunk.token_estimate as i64,
                    chunk.text,
                ])?;
            }
        }

        let removed = tx.query_row(
            "select count(*)
             from semantic_chunks sc
             left join temp.desired_semantic_chunks d
                on d.file_id = sc.file_id
                and d.start_line = sc.start_line
                and d.end_line = sc.end_line
             where d.file_id is null",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let updated = tx.query_row(
            "select count(*)
             from semantic_chunks sc
             join temp.desired_semantic_chunks d
                on d.file_id = sc.file_id
                and d.start_line = sc.start_line
                and d.end_line = sc.end_line
             where d.content_hash != sc.content_hash",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let added = tx.query_row(
            "select count(*)
             from temp.desired_semantic_chunks d
             left join semantic_chunks sc
                on sc.file_id = d.file_id
                and sc.start_line = d.start_line
                and sc.end_line = d.end_line
             where sc.id is null",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let changes = if explain_changes {
            let mut stmt = tx.prepare(
                "select change, file, start_line, end_line, previous_hash, content_hash
                 from (
                    select
                        'removed' as change,
                        f.path as file,
                        sc.start_line as start_line,
                        sc.end_line as end_line,
                        sc.content_hash as previous_hash,
                        null as content_hash
                    from semantic_chunks sc
                    join files f on f.id = sc.file_id
                    left join temp.desired_semantic_chunks d
                        on d.file_id = sc.file_id
                        and d.start_line = sc.start_line
                        and d.end_line = sc.end_line
                    where d.file_id is null
                    union all
                    select
                        'updated' as change,
                        f.path as file,
                        sc.start_line as start_line,
                        sc.end_line as end_line,
                        sc.content_hash as previous_hash,
                        d.content_hash as content_hash
                    from semantic_chunks sc
                    join files f on f.id = sc.file_id
                    join temp.desired_semantic_chunks d
                        on d.file_id = sc.file_id
                        and d.start_line = sc.start_line
                        and d.end_line = sc.end_line
                    where d.content_hash != sc.content_hash
                    union all
                    select
                        'added' as change,
                        f.path as file,
                        d.start_line as start_line,
                        d.end_line as end_line,
                        null as previous_hash,
                        d.content_hash as content_hash
                    from temp.desired_semantic_chunks d
                    join files f on f.id = d.file_id
                    left join semantic_chunks sc
                        on sc.file_id = d.file_id
                        and sc.start_line = d.start_line
                        and sc.end_line = d.end_line
                    where sc.id is null
                 )
                 order by file, start_line, end_line, change",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(SemanticChunkChange {
                    change: row.get(0)?,
                    file: row.get(1)?,
                    start_line: row.get::<_, i64>(2)? as usize,
                    end_line: row.get::<_, i64>(3)? as usize,
                    previous_hash: row.get(4)?,
                    content_hash: row.get(5)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        tx.execute(
            "delete from semantic_embeddings
             where chunk_id in (
                select sc.id
                from semantic_chunks sc
                left join temp.desired_semantic_chunks d
                    on d.file_id = sc.file_id
                    and d.start_line = sc.start_line
                    and d.end_line = sc.end_line
                where d.file_id is null
                    or d.content_hash != sc.content_hash
             )",
            [],
        )?;
        tx.execute(
            "delete from semantic_chunks
             where id in (
                select sc.id
                from semantic_chunks sc
                left join temp.desired_semantic_chunks d
                    on d.file_id = sc.file_id
                    and d.start_line = sc.start_line
                    and d.end_line = sc.end_line
                where d.file_id is null
             )",
            [],
        )?;
        tx.execute(
            "update semantic_chunks
             set
                content_hash = (
                    select d.content_hash
                    from temp.desired_semantic_chunks d
                    where d.file_id = semantic_chunks.file_id
                        and d.start_line = semantic_chunks.start_line
                        and d.end_line = semantic_chunks.end_line
                ),
                token_estimate = (
                    select d.token_estimate
                    from temp.desired_semantic_chunks d
                    where d.file_id = semantic_chunks.file_id
                        and d.start_line = semantic_chunks.start_line
                        and d.end_line = semantic_chunks.end_line
                ),
                text = (
                    select d.text
                    from temp.desired_semantic_chunks d
                    where d.file_id = semantic_chunks.file_id
                        and d.start_line = semantic_chunks.start_line
                        and d.end_line = semantic_chunks.end_line
                ),
                updated_at = ?1
             where exists (
                select 1
                from temp.desired_semantic_chunks d
                where d.file_id = semantic_chunks.file_id
                    and d.start_line = semantic_chunks.start_line
                    and d.end_line = semantic_chunks.end_line
                    and d.content_hash != semantic_chunks.content_hash
             )",
            params![updated_at],
        )?;
        tx.execute(
            "insert into semantic_chunks
             (file_id, start_line, end_line, content_hash, token_estimate, text, updated_at)
             select d.file_id, d.start_line, d.end_line, d.content_hash, d.token_estimate, d.text, ?1
             from temp.desired_semantic_chunks d
             left join semantic_chunks sc
                on sc.file_id = d.file_id
                and sc.start_line = d.start_line
                and sc.end_line = d.end_line
             where sc.id is null",
            params![updated_at],
        )?;
        tx.execute("drop table if exists temp.desired_semantic_chunks", [])?;
        let total = tx.query_row("select count(*) from semantic_chunks", [], |row| {
            row.get::<_, i64>(0)
        })? as usize;
        tx.commit()?;
        Ok(SemanticChunkWriteStats {
            total,
            added,
            updated,
            removed,
            changes,
        })
    }

    pub fn count_semantic_chunks(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("select count(*) from semantic_chunks", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn count_semantic_embeddings_for(&self, provider: &str, model: &str) -> Result<usize> {
        Ok(self.conn.query_row(
            "select count(*) from semantic_embeddings where provider = ?1 and model = ?2",
            params![provider, model],
            |row| row.get::<_, i64>(0),
        )? as usize)
    }

    #[cfg(test)]
    pub fn semantic_chunks(&self) -> Result<Vec<SemanticChunk>> {
        let mut stmt = self.conn.prepare(
            "select sc.id, f.path, sc.start_line, sc.end_line, sc.token_estimate, sc.text
             from semantic_chunks sc
             join files f on f.id = sc.file_id
             order by f.path, sc.start_line",
        )?;
        let rows = stmt.query_map([], semantic_chunk_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn semantic_chunks_missing_embeddings(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<Vec<SemanticChunk>> {
        let mut stmt = self.conn.prepare(
            "select sc.id, f.path, sc.start_line, sc.end_line, sc.token_estimate, sc.text
             from semantic_chunks sc
             join files f on f.id = sc.file_id
             left join semantic_embeddings se
                on se.chunk_id = sc.id
                and se.provider = ?1
                and se.model = ?2
             where se.id is null
             order by f.path, sc.start_line",
        )?;
        let rows = stmt.query_map(params![provider, model], semantic_chunk_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn upsert_semantic_embeddings(
        &mut self,
        provider: &str,
        model: &str,
        embeddings: &[SemanticEmbeddingInput],
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let updated_at = unix_timestamp();
        let mut written = 0;
        {
            let mut stmt = tx.prepare(
                "insert into semantic_embeddings
                 (chunk_id, provider, model, dimensions, vector, updated_at)
                 values (?1, ?2, ?3, ?4, ?5, ?6)
                 on conflict(chunk_id, provider, model) do update set
                    dimensions = excluded.dimensions,
                    vector = excluded.vector,
                    updated_at = excluded.updated_at",
            )?;
            for embedding in embeddings {
                written += stmt.execute(params![
                    embedding.chunk_id,
                    provider,
                    model,
                    embedding.vector.len() as i64,
                    encode_f32_vector(&embedding.vector),
                    updated_at,
                ])?;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    pub fn semantic_embedding_matches(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<Vec<SemanticEmbeddingMatch>> {
        let mut stmt = self.conn.prepare(
            "select sc.id, f.path, sc.start_line, sc.end_line, sc.token_estimate, sc.text,
                    se.dimensions, se.vector
             from semantic_embeddings se
             join semantic_chunks sc on sc.id = se.chunk_id
             join files f on f.id = sc.file_id
             where se.provider = ?1 and se.model = ?2
             order by f.path, sc.start_line",
        )?;
        let mut rows = stmt.query(params![provider, model])?;
        let mut matches = Vec::new();
        while let Some(row) = rows.next()? {
            let dimensions = row.get::<_, i64>(6)? as usize;
            let vector_blob = row.get::<_, Vec<u8>>(7)?;
            matches.push(SemanticEmbeddingMatch {
                chunk: SemanticChunk {
                    id: row.get(0)?,
                    file: row.get(1)?,
                    start_line: row.get::<_, i64>(2)? as usize,
                    end_line: row.get::<_, i64>(3)? as usize,
                    token_estimate: row.get::<_, i64>(4)? as usize,
                    text: row.get(5)?,
                },
                vector: decode_f32_vector(&vector_blob, dimensions)?,
            });
        }
        Ok(matches)
    }

    pub fn semantic_chunks_matching(
        &self,
        terms: &[String],
        limit: usize,
    ) -> Result<Vec<SemanticChunk>> {
        let terms = terms
            .iter()
            .map(|term| term.trim().to_ascii_lowercase())
            .filter(|term| term.len() >= 3)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .take(16)
            .collect::<Vec<_>>();
        if terms.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let conditions = (0..terms.len())
            .map(|index| {
                let placeholder = index + 1;
                format!("lower(sc.text) like ?{placeholder}")
            })
            .collect::<Vec<_>>()
            .join(" or ");
        let sql = format!(
            "select sc.id, f.path, sc.start_line, sc.end_line, sc.token_estimate, sc.text
             from semantic_chunks sc
             join files f on f.id = sc.file_id
             where {conditions}
             order by f.path, sc.start_line"
        );
        let patterns = terms
            .iter()
            .map(|term| format!("%{term}%"))
            .collect::<Vec<_>>();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(patterns.iter()), semantic_chunk_from_row)?;

        let mut chunks = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        chunks.truncate(limit);
        Ok(chunks)
    }

    pub fn overview(&self, root: &Path) -> Result<ProjectOverview> {
        let indexed_files = self
            .conn
            .query_row("select count(*) from files", [], |row| row.get::<_, i64>(0))?
            as usize;
        let total_lines = self.conn.query_row(
            "select coalesce(sum(line_count), 0) from files",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let symbols = self
            .conn
            .query_row("select count(*) from symbols", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
        let dependencies = self
            .conn
            .query_row("select count(*) from dependencies", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
        let call_edges = self
            .conn
            .query_row("select count(*) from calls", [], |row| row.get::<_, i64>(0))?
            as usize;

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

        let mut main_dir_stmt = self.conn.prepare(
            "select
                case
                  when instr(f.path, '/') = 0 then '.'
                  else substr(f.path, 1, instr(f.path, '/') - 1)
                end as directory,
                count(*),
                coalesce(sum(f.line_count), 0),
                coalesce(sum(fs.symbols), 0)
             from files f
             left join (
                select file_id, count(*) as symbols
                from symbols
                group by file_id
             ) fs on fs.file_id = f.id
             group by directory
             order by count(*) desc, coalesce(sum(f.line_count), 0) desc, directory
             limit 12",
        )?;
        let main_directories = main_dir_stmt
            .query_map([], |row| {
                let directory = row.get::<_, String>(0)?;
                Ok(DirectorySummary {
                    role: path_role(&directory).to_string(),
                    directory,
                    files: row.get::<_, i64>(1)? as usize,
                    lines: row.get::<_, i64>(2)? as usize,
                    symbols: row.get::<_, i64>(3)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut symbol_kind_stmt = self.conn.prepare(
            "select kind, count(*)
             from symbols
             group by kind
             order by count(*) desc, kind",
        )?;
        let symbol_kinds = symbol_kind_stmt
            .query_map([], |row| {
                Ok(SymbolKindStat {
                    kind: row.get(0)?,
                    symbols: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let resolved_dependencies = self.conn.query_row(
            "select count(*) from dependencies where resolved_file is not null",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let local_dependencies = self.conn.query_row(
            "select count(*)
             from dependencies
             where resolved_file is not null and resolved_file not like 'node_modules/%'",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let unresolved_dependencies = dependencies.saturating_sub(resolved_dependencies);
        let external_targets = self.conn.query_row(
            "select count(distinct target)
             from dependencies
             where resolved_file is null or resolved_file like 'node_modules/%'",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let mut external_target_stmt = self.conn.prepare(
            "select target, count(*)
             from dependencies
             where resolved_file is null or resolved_file like 'node_modules/%'
             group by target
             order by count(*) desc, target
             limit 12",
        )?;
        let top_external_targets = external_target_stmt
            .query_map([], |row| {
                Ok(DependencyTargetStat {
                    target: row.get(0)?,
                    edges: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let type_relation_edges = self.conn.query_row(
            "select count(*) from dependencies where kind = 'base_type'",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let mut type_relation_target_stmt = self.conn.prepare(
            "select target, count(*)
             from dependencies
             where kind = 'base_type'
             group by target
             order by count(*) desc, target
             limit 12",
        )?;
        let top_type_relation_targets = type_relation_target_stmt
            .query_map([], |row| {
                Ok(DependencyTargetStat {
                    target: row.get(0)?,
                    edges: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let dependency_summary = DependencySummary {
            edges: dependencies,
            local_edges: local_dependencies,
            external_edges: dependencies.saturating_sub(local_dependencies),
            resolved_edges: resolved_dependencies,
            unresolved_edges: unresolved_dependencies,
            type_relation_edges,
            external_targets,
            top_external_targets,
            top_type_relation_targets,
        };

        let resolved_callee_edges = self.conn.query_row(
            "select count(*) from calls where callee_file is not null",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let call_summary = CallSummary {
            edges: call_edges,
            resolved_callee_edges,
            unresolved_callee_edges: call_edges.saturating_sub(resolved_callee_edges),
        };

        let entrypoints = self.entrypoint_candidates()?;
        let index_status = IndexStatus {
            schema_version: self
                .get_meta("schema_version")?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(SCHEMA_VERSION),
            index_version: self
                .get_meta("index_version")?
                .unwrap_or_else(|| INDEX_VERSION.to_string()),
            last_indexed_at: self
                .get_meta("last_indexed_at")?
                .and_then(|value| value.parse::<i64>().ok()),
        };
        let summary = overview_summary(
            indexed_files,
            total_lines,
            symbols,
            dependencies,
            type_relation_edges,
            call_edges,
            &languages,
            &top_directories,
        );
        let recommended_next_tools =
            recommended_next_tools(root, &entrypoints, &dependency_summary, &call_summary);

        Ok(ProjectOverview {
            root: root.display().to_string(),
            indexed_files,
            total_lines,
            symbols,
            dependencies,
            call_edges,
            summary,
            languages,
            top_directories,
            main_directories,
            symbol_kinds,
            dependency_summary,
            call_summary,
            entrypoints,
            recommended_next_tools,
            index_status,
        })
    }

    fn entrypoint_candidates(&self) -> Result<Vec<EntryPointCandidate>> {
        let mut stmt = self.conn.prepare(
            "select f.path, f.language, s.name
             from files f
             left join symbols s on s.file_id = f.id
                and s.kind in ('function', 'method')
                and lower(s.name) in ('main', 'run', 'start', 'server', 'handler')
             order by f.path, s.start_line",
        )?;
        let mut candidates = BTreeMap::<String, EntryPointCandidate>::new();
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        for row in rows {
            let (file, language, symbol) = row?;
            if let Some((score, reason)) = file_entrypoint_signal(&file) {
                upsert_entrypoint_candidate(&mut candidates, &file, &language, score, reason, None);
            }
            if let Some(symbol) = symbol
                && let Some((score, reason)) = symbol_entrypoint_signal(&symbol)
            {
                upsert_entrypoint_candidate(
                    &mut candidates,
                    &file,
                    &language,
                    score,
                    reason,
                    Some(symbol),
                );
            }
        }

        let mut candidates = candidates.into_values().collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            entrypoint_sort_score(right)
                .cmp(&entrypoint_sort_score(left))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        });
        candidates.truncate(12);
        for candidate in &mut candidates {
            candidate.confidence = entrypoint_confidence(candidate.score);
        }
        Ok(candidates)
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

    pub fn symbols_for_files(&self, files: &[String], limit: usize) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        let mut stmt = self.conn.prepare(
            "select s.name, s.qualified_name, s.kind, s.language, f.path, s.start_line, s.end_line
             from symbols s
             join files f on f.id = s.file_id
             where f.path = ?1
             order by s.start_line, s.end_line
             limit ?2",
        )?;

        for file in files {
            if symbols.len() >= limit {
                break;
            }
            let remaining = limit.saturating_sub(symbols.len());
            let rows = stmt.query_map(params![file, remaining as i64], |row| {
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
            symbols.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }

        Ok(symbols)
    }

    pub fn dependency_graph(
        &self,
        root: &Path,
        limit: usize,
        offset: usize,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<DependencyGraph> {
        let mut query_params = Vec::new();
        let where_clause =
            dependency_graph_filter_clause(files, languages, kinds, &mut query_params);
        let query = format!(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}
             order by f.path, d.line
             limit ? offset ?"
        );
        let page_query_limit = limit.saturating_add(1);
        query_params.push(SqlValue::Integer(page_query_limit as i64));
        query_params.push(SqlValue::Integer(offset as i64));
        let mut stmt = self.conn.prepare(&query)?;
        let mut dependencies = stmt
            .query_map(params_from_iter(query_params), |row| {
                let language: String = row.get(6)?;
                Ok(Dependency {
                    source_file: row.get(0)?,
                    target: row.get(1)?,
                    resolved_file: row.get(2)?,
                    local_alias: row.get(3)?,
                    imported_symbol: row.get(4)?,
                    kind: row.get(5)?,
                    language: parse_language(&language),
                    line: row.get::<_, i64>(7)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = dependencies.len() > limit;
        if has_more {
            dependencies.truncate(limit);
        }
        let page_size = dependencies.len();

        let (nodes, edges) = if files.is_empty() && languages.is_empty() && kinds.is_empty() {
            let nodes = self.conn.query_row(
                "select count(*) from (
                    select path from files
                    union
                    select target from dependencies
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )? as usize;
            let edges = self
                .conn
                .query_row("select count(*) from dependencies", [], |row| {
                    row.get::<_, i64>(0)
                })? as usize;
            (nodes, edges)
        } else {
            let mut edge_params = Vec::new();
            let edge_where =
                dependency_graph_filter_clause(files, languages, kinds, &mut edge_params);
            let edge_query = format!(
                "select count(*)
                 from dependencies d
                 join files f on f.id = d.source_file_id
                 {edge_where}"
            );
            let edges = self
                .conn
                .query_row(&edge_query, params_from_iter(edge_params), |row| {
                    row.get::<_, i64>(0)
                })? as usize;

            let mut node_params = Vec::new();
            let source_where =
                dependency_graph_filter_clause(files, languages, kinds, &mut node_params);
            let target_where =
                dependency_graph_filter_clause(files, languages, kinds, &mut node_params);
            let node_query = format!(
                "select count(*) from (
                    select f.path
                    from dependencies d
                    join files f on f.id = d.source_file_id
                    {source_where}
                    union
                    select d.target
                    from dependencies d
                    join files f on f.id = d.source_file_id
                    {target_where}
                 )"
            );
            let nodes = self
                .conn
                .query_row(&node_query, params_from_iter(node_params), |row| {
                    row.get::<_, i64>(0)
                })? as usize;
            (nodes, edges)
        };

        let summary = self.dependency_graph_summary(files, languages, kinds)?;
        let top_sources = self.dependency_graph_top_sources(files, languages, kinds)?;
        let top_targets = self.dependency_graph_top_targets(files, languages, kinds)?;

        Ok(DependencyGraph {
            root: root.display().to_string(),
            dependencies,
            nodes,
            edges,
            limit,
            offset,
            page_size,
            has_more,
            summary,
            top_sources,
            top_targets,
        })
    }

    fn dependency_graph_summary(
        &self,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<DependencySummary> {
        let mut params = Vec::new();
        let where_clause = dependency_graph_filter_clause(files, languages, kinds, &mut params);
        let query = format!(
            "select
                count(*),
                coalesce(sum(case when d.resolved_file is not null and d.resolved_file not like 'node_modules/%' then 1 else 0 end), 0),
                coalesce(sum(case when d.resolved_file is not null then 1 else 0 end), 0),
                coalesce(sum(case when d.kind = 'base_type' then 1 else 0 end), 0),
                count(distinct case when d.resolved_file is null or d.resolved_file like 'node_modules/%' then d.target end)
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}"
        );
        let (edges, local_edges, resolved_edges, type_relation_edges, external_targets) = self
            .conn
            .query_row(&query, params_from_iter(params), |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, i64>(3)? as usize,
                    row.get::<_, i64>(4)? as usize,
                ))
            })?;
        let top_external_targets =
            self.dependency_graph_top_external_targets(files, languages, kinds)?;
        let top_type_relation_targets =
            self.dependency_graph_top_type_relation_targets(files, languages, kinds)?;

        Ok(DependencySummary {
            edges,
            local_edges,
            external_edges: edges.saturating_sub(local_edges),
            resolved_edges,
            unresolved_edges: edges.saturating_sub(resolved_edges),
            type_relation_edges,
            external_targets,
            top_external_targets,
            top_type_relation_targets,
        })
    }

    fn dependency_graph_top_sources(
        &self,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<Vec<DependencySourceStat>> {
        let mut params = Vec::new();
        let where_clause = dependency_graph_filter_clause(files, languages, kinds, &mut params);
        let query = format!(
            "select f.path, count(*)
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}
             group by f.path
             order by count(*) desc, f.path
             limit 12"
        );
        let mut stmt = self.conn.prepare(&query)?;
        stmt.query_map(params_from_iter(params), |row| {
            Ok(DependencySourceStat {
                source_file: row.get(0)?,
                edges: row.get::<_, i64>(1)? as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
    }

    fn dependency_graph_top_targets(
        &self,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<Vec<DependencyTargetStat>> {
        let mut params = Vec::new();
        let where_clause = dependency_graph_filter_clause(files, languages, kinds, &mut params);
        let query = format!(
            "select d.target, count(*)
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}
             group by d.target
             order by count(*) desc, d.target
             limit 12"
        );
        let mut stmt = self.conn.prepare(&query)?;
        stmt.query_map(params_from_iter(params), |row| {
            Ok(DependencyTargetStat {
                target: row.get(0)?,
                edges: row.get::<_, i64>(1)? as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
    }

    fn dependency_graph_top_external_targets(
        &self,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<Vec<DependencyTargetStat>> {
        let mut params = Vec::new();
        let base_where = dependency_graph_filter_clause(files, languages, kinds, &mut params);
        let external_condition =
            "(d.resolved_file is null or d.resolved_file like 'node_modules/%')";
        let where_clause = if base_where.is_empty() {
            format!("where {external_condition}")
        } else {
            format!("{base_where} and {external_condition}")
        };
        let query = format!(
            "select d.target, count(*)
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}
             group by d.target
             order by count(*) desc, d.target
             limit 12"
        );
        let mut stmt = self.conn.prepare(&query)?;
        stmt.query_map(params_from_iter(params), |row| {
            Ok(DependencyTargetStat {
                target: row.get(0)?,
                edges: row.get::<_, i64>(1)? as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
    }

    fn dependency_graph_top_type_relation_targets(
        &self,
        files: &[String],
        languages: &[String],
        kinds: &[String],
    ) -> Result<Vec<DependencyTargetStat>> {
        let mut params = Vec::new();
        let base_where = dependency_graph_filter_clause(files, languages, kinds, &mut params);
        let relation_condition = "d.kind = 'base_type'";
        let where_clause = if base_where.is_empty() {
            format!("where {relation_condition}")
        } else {
            format!("{base_where} and {relation_condition}")
        };
        let query = format!(
            "select d.target, count(*)
             from dependencies d
             join files f on f.id = d.source_file_id
             {where_clause}
             group by d.target
             order by count(*) desc, d.target
             limit 12"
        );
        let mut stmt = self.conn.prepare(&query)?;
        stmt.query_map(params_from_iter(params), |row| {
            Ok(DependencyTargetStat {
                target: row.get(0)?,
                edges: row.get::<_, i64>(1)? as usize,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
    }

    pub fn dependencies_touching_files(
        &self,
        files: &[String],
        limit: usize,
    ) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             where f.path = ?1 or d.resolved_file = ?1
             order by f.path, d.line
             limit ?2",
        )?;

        for file in files {
            if dependencies.len() >= limit {
                break;
            }
            let remaining = limit.saturating_sub(dependencies.len());
            let rows = stmt.query_map(params![file, remaining as i64], |row| {
                let language: String = row.get(6)?;
                Ok(Dependency {
                    source_file: row.get(0)?,
                    target: row.get(1)?,
                    resolved_file: row.get(2)?,
                    local_alias: row.get(3)?,
                    imported_symbol: row.get(4)?,
                    kind: row.get(5)?,
                    language: parse_language(&language),
                    line: row.get::<_, i64>(7)? as usize,
                })
            })?;
            dependencies.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }

        Ok(dependencies)
    }

    pub fn dependency_importers_for_files(
        &self,
        files: &[String],
        limit: usize,
    ) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             where d.resolved_file = ?1
             order by f.path, d.line
             limit ?2",
        )?;

        for file in files {
            if dependencies.len() >= limit {
                break;
            }
            let remaining = limit.saturating_sub(dependencies.len());
            let rows = stmt.query_map(params![file, remaining as i64], |row| {
                let language: String = row.get(6)?;
                Ok(Dependency {
                    source_file: row.get(0)?,
                    target: row.get(1)?,
                    resolved_file: row.get(2)?,
                    local_alias: row.get(3)?,
                    imported_symbol: row.get(4)?,
                    kind: row.get(5)?,
                    language: parse_language(&language),
                    line: row.get::<_, i64>(7)? as usize,
                })
            })?;
            dependencies.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }

        Ok(dependencies)
    }

    pub fn type_relation_importers_for_symbols(
        &self,
        symbols: &[String],
        limit: usize,
    ) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             where d.kind = 'base_type'
               and (d.target = ?1 or d.target like ?2)
             order by f.path, d.line
             limit ?3",
        )?;

        for symbol in symbols {
            if dependencies.len() >= limit {
                break;
            }
            let symbol = symbol.trim();
            if symbol.is_empty() {
                continue;
            }
            let remaining = limit.saturating_sub(dependencies.len());
            let suffix = format!("%.{}", symbol);
            let rows = stmt.query_map(params![symbol, suffix, remaining as i64], |row| {
                let language: String = row.get(6)?;
                Ok(Dependency {
                    source_file: row.get(0)?,
                    target: row.get(1)?,
                    resolved_file: row.get(2)?,
                    local_alias: row.get(3)?,
                    imported_symbol: row.get(4)?,
                    kind: row.get(5)?,
                    language: parse_language(&language),
                    line: row.get::<_, i64>(7)? as usize,
                })
            })?;
            for dependency in rows {
                let dependency = dependency?;
                let key = (
                    dependency.source_file.clone(),
                    dependency.target.clone(),
                    dependency.local_alias.clone(),
                    dependency.line,
                );
                if seen.insert(key) {
                    dependencies.push(dependency);
                }
            }
        }

        Ok(dependencies)
    }

    pub fn resolved_dependencies_for_files(&self, files: &[String]) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             where f.path = ?1 and d.resolved_file is not null
             order by d.line",
        )?;

        for file in files {
            let rows = stmt.query_map(params![file], |row| {
                let language: String = row.get(6)?;
                Ok(Dependency {
                    source_file: row.get(0)?,
                    target: row.get(1)?,
                    resolved_file: row.get(2)?,
                    local_alias: row.get(3)?,
                    imported_symbol: row.get(4)?,
                    kind: row.get(5)?,
                    language: parse_language(&language),
                    line: row.get::<_, i64>(7)? as usize,
                })
            })?;
            dependencies.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }

        Ok(dependencies)
    }

    pub fn callers(&self, symbol: &str, limit: usize) -> Result<Vec<CallEdge>> {
        self.call_edges(
            "select f.path, c.caller, c.callee, c.callee_file, c.language, c.line, c.column, c.confidence
             from calls c
             join files f on f.id = c.source_file_id
             where c.callee = ?1 or c.callee like ?2
             order by f.path, c.line
             limit ?3",
            symbol,
            format!("%.{}", symbol),
            limit,
        )
    }

    pub fn callees(&self, symbol: &str, limit: usize) -> Result<Vec<CallEdge>> {
        self.call_edges(
            "select f.path, c.caller, c.callee, c.callee_file, c.language, c.line, c.column, c.confidence
             from calls c
             join files f on f.id = c.source_file_id
             where c.caller = ?1 or c.caller like ?2
             order by f.path, c.line
             limit ?3",
            symbol,
            format!("%.{}", symbol),
            limit,
        )
    }

    fn call_edges(
        &self,
        sql: &str,
        exact: &str,
        suffix: String,
        limit: usize,
    ) -> Result<Vec<CallEdge>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![exact, suffix, limit as i64], |row| {
            let language: String = row.get(4)?;
            Ok(CallEdge {
                file: row.get(0)?,
                caller: row.get(1)?,
                callee: row.get(2)?,
                callee_file: row.get(3)?,
                language: parse_language(&language),
                line: row.get::<_, i64>(5)? as usize,
                column: row.get::<_, i64>(6)? as usize,
                confidence: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn resolve_imported_calls(&self) -> Result<usize> {
        self.conn.execute_batch(
            "
            drop table if exists temp.imported_call_targets;
            drop table if exists temp.csharp_type_bound_calls;
            drop table if exists temp.csharp_property_call_parts;

            create temp table csharp_type_bound_calls as
            select
                c.id as call_id,
                c.source_file_id,
                type_bindings.target,
                type_bindings.local_alias,
                type_bindings.imported_symbol,
                type_bindings.line as dependency_line,
                case
                    when type_bindings.imported_symbol is not null
                      and type_bindings.imported_symbol != '*'
                      and c.callee like
                        type_bindings.local_alias || '.' || type_bindings.imported_symbol || '.%'
                    then substr(
                        c.callee,
                        length(type_bindings.local_alias) +
                        length(type_bindings.imported_symbol) +
                        3
                    )
                    else substr(c.callee, length(type_bindings.local_alias) + 2)
                end as member_tail
            from calls c
            join dependencies type_bindings
              on type_bindings.source_file_id = c.source_file_id
            where c.callee_file is null
              and c.language = 'csharp'
              and type_bindings.language = 'csharp'
              and type_bindings.kind = 'type_binding'
              and type_bindings.local_alias is not null
              and c.callee like type_bindings.local_alias || '.%';

            create temp table csharp_property_call_parts as
            select
                c.call_id,
                owner_files.id as owner_file_id,
                owner_files.path as owner_file,
                property_types.target as property_type,
                c.dependency_line,
                substr(
                    c.member_tail,
                    instr(c.member_tail, '.') + 1
                ) as member_tail
            from temp.csharp_type_bound_calls c
            join dependencies scopes
              on scopes.source_file_id = c.source_file_id
            join files owner_files
              on owner_files.path like '%' ||
                replace(scopes.target, '.', '/') ||
                '/' ||
                c.target ||
                '.cs'
            join dependencies property_types
              on property_types.source_file_id = owner_files.id
            where instr(c.member_tail, '.') > 0
              and scopes.language = 'csharp'
              and scopes.kind in ('using', 'namespace')
              and property_types.language = 'csharp'
              and property_types.kind = 'property_type'
              and property_types.local_alias is not null
              and property_types.imported_symbol is not null
              and (
                property_types.imported_symbol = c.target
                or c.target like '%.' || property_types.imported_symbol
              )
              and property_types.local_alias = substr(
                c.member_tail,
                1,
                instr(c.member_tail, '.') - 1
              )

            union all

            select
                c.call_id,
                owner_files.id as owner_file_id,
                owner_files.path as owner_file,
                property_types.target as property_type,
                c.dependency_line,
                substr(
                    c.member_tail,
                    instr(c.member_tail, '.') + 1
                ) as member_tail
            from temp.csharp_type_bound_calls c
            join files owner_files
              on owner_files.path like '%' ||
                replace(c.target, '.', '/') ||
                '.cs'
            join dependencies property_types
              on property_types.source_file_id = owner_files.id
            where instr(c.member_tail, '.') > 0
              and c.target like '%.%'
              and property_types.language = 'csharp'
              and property_types.kind = 'property_type'
              and property_types.local_alias is not null
              and property_types.imported_symbol is not null
              and (
                property_types.imported_symbol = c.target
                or c.target like '%.' || property_types.imported_symbol
              )
              and property_types.local_alias = substr(
                c.member_tail,
                1,
                instr(c.member_tail, '.') - 1
              )

            union all

            select
                c.call_id,
                owner_files.id as owner_file_id,
                owner_files.path as owner_file,
                property_types.target as property_type,
                c.dependency_line,
                substr(
                    c.member_tail,
                    instr(c.member_tail, '.') + 1
                ) as member_tail
            from temp.csharp_type_bound_calls c
            join dependencies aliases
              on aliases.source_file_id = c.source_file_id
            join files owner_files
              on owner_files.path like '%' ||
                replace(aliases.target, '.', '/') ||
                '.cs'
            join dependencies property_types
              on property_types.source_file_id = owner_files.id
            where instr(c.member_tail, '.') > 0
              and aliases.language = 'csharp'
              and aliases.kind = 'using_alias'
              and aliases.local_alias = c.target
              and property_types.language = 'csharp'
              and property_types.kind = 'property_type'
              and property_types.local_alias is not null
              and property_types.imported_symbol is not null
              and (
                property_types.imported_symbol = aliases.target
                or aliases.target like '%.' || property_types.imported_symbol
              )
              and property_types.local_alias = substr(
                c.member_tail,
                1,
                instr(c.member_tail, '.') - 1
              );

            create temp table imported_call_targets as
            select call_id, callee_file
            from (
                select
                    call_id,
                    callee_file,
                    row_number() over (
                        partition by call_id
                        order by match_rank, length(qualified_name), dependency_line, start_line
                    ) as target_rank
                from (
                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        case
                            when d.local_alias = c.callee
                              and d.imported_symbol is not null
                              and (
                                s.name = d.imported_symbol
                                or s.qualified_name = d.imported_symbol
                                or s.qualified_name like '%.' || d.imported_symbol
                              )
                                then 0
                            when d.imported_symbol = '*'
                              and c.callee like d.local_alias || '.%'
                              and (
                                s.name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                              )
                                then 0
                            when d.local_alias is not null
                              and c.callee like d.local_alias || '.%'
                              and (
                                s.name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                              )
                                then 0
                            when c.callee like 'require.%'
                              and d.line = c.line
                              and (
                                s.name = substr(c.callee, length('require') + 2)
                                or s.qualified_name = substr(c.callee, length('require') + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length('require') + 2)
                              )
                                then 0
                            when d.language = 'csharp'
                              and d.kind = 'using_static'
                              and s.name = c.callee
                                then 0
                            when d.language = 'csharp'
                              and d.kind in ('using', 'namespace')
                              and s.name = c.callee
                                then 2
                            when s.name = c.callee then 1
                            else 2
                        end as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and (d.language != 'go' or s.kind = 'function')
                      and (
                        s.name = c.callee
                        or s.qualified_name = c.callee
                        or s.qualified_name like '%.' || c.callee
                        or (
                            d.local_alias = c.callee
                            and d.imported_symbol is not null
                            and (
                                s.name = d.imported_symbol
                                or s.qualified_name = d.imported_symbol
                                or s.qualified_name like '%.' || d.imported_symbol
                            )
                        )
                        or (
                            d.imported_symbol = '*'
                            and c.callee like d.local_alias || '.%'
                            and (
                                s.name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                            )
                        )
                        or (
                            d.local_alias is not null
                            and c.callee like d.local_alias || '.%'
                            and (
                                s.name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                            )
                        )
                        or (
                            c.callee like 'require.%'
                            and d.line = c.line
                            and (
                                s.name = substr(c.callee, length('require') + 2)
                                or s.qualified_name = substr(c.callee, length('require') + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length('require') + 2)
                            )
                        )
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        0 as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join symbols s
                      on s.language = 'csharp'
                    join files target_files
                      on target_files.id = s.file_id
                      and target_files.path like '%' ||
                        replace(
                          substr(c.callee, 1, length(c.callee) - length(s.name) - 1),
                          '.',
                          '/'
                        ) ||
                        '.cs'
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and c.callee like '%.%'
                      and instr(substr(c.callee, instr(c.callee, '.') + 1), '.') > 0
                      and c.callee like '%.' || s.name

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(substr(d.target, 1, length(d.target) - 2), '.', '/') ||
                        '/' ||
                        substr(c.callee, 1, instr(c.callee, '.') - 1) ||
                        '.java'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and d.language = 'java'
                      and d.kind = 'import'
                      and d.target like '%.%.*'
                      and instr(c.callee, '.') > 1
                      and (
                        s.name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name like '%.' || substr(c.callee, instr(c.callee, '.') + 1)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(d.target, '.', '/') ||
                        '/' ||
                        substr(c.callee, 1, instr(c.callee, '.') - 1) ||
                        '.java'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and d.language = 'java'
                      and d.kind = 'package'
                      and instr(c.callee, '.') > 1
                      and (
                        s.name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name like '%.' || substr(c.callee, instr(c.callee, '.') + 1)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        1 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(d.target, '.', '/') ||
                        '/' ||
                        substr(c.callee, 1, instr(c.callee, '.') - 1) ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and d.language = 'csharp'
                      and d.kind = 'namespace'
                      and instr(c.callee, '.') > 1
                      and (
                        s.name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name = substr(c.callee, instr(c.callee, '.') + 1)
                        or s.qualified_name like '%.' || substr(c.callee, instr(c.callee, '.') + 1)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        base_types.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies base_types
                      on base_types.source_file_id = c.source_file_id
                    join dependencies scopes
                      on scopes.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(scopes.target, '.', '/') ||
                        '/' ||
                        base_types.target ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and base_types.language = 'csharp'
                      and base_types.kind = 'base_type'
                      and base_types.local_alias is not null
                      and scopes.language = 'csharp'
                      and scopes.kind in ('using', 'namespace')
                      and c.caller like base_types.local_alias || '.%'
                      and c.callee like 'base.%'
                      and (
                        s.name = substr(c.callee, length('base') + 2)
                        or s.qualified_name = substr(c.callee, length('base') + 2)
                        or s.qualified_name like '%.' || substr(c.callee, length('base') + 2)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        base_types.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies base_types
                      on base_types.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(base_types.target, '.', '/') ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and base_types.language = 'csharp'
                      and base_types.kind = 'base_type'
                      and base_types.local_alias is not null
                      and base_types.target like '%.%'
                      and c.caller like base_types.local_alias || '.%'
                      and c.callee like 'base.%'
                      and (
                        s.name = substr(c.callee, length('base') + 2)
                        or s.qualified_name = substr(c.callee, length('base') + 2)
                        or s.qualified_name like '%.' || substr(c.callee, length('base') + 2)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        inherited_base_types.line as dependency_line,
                        s.start_line as start_line,
                        1 as match_rank
                    from calls c
                    join dependencies base_types
                      on base_types.source_file_id = c.source_file_id
                    join files base_files
                      on base_files.path like '%' ||
                        replace(base_types.target, '.', '/') ||
                        '.cs'
                    join dependencies inherited_base_types
                      on inherited_base_types.source_file_id = base_files.id
                    join dependencies inherited_scopes
                      on inherited_scopes.source_file_id = base_files.id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(inherited_scopes.target, '.', '/') ||
                        '/' ||
                        inherited_base_types.target ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and base_types.language = 'csharp'
                      and base_types.kind = 'base_type'
                      and base_types.local_alias is not null
                      and base_types.target like '%.%'
                      and inherited_base_types.language = 'csharp'
                      and inherited_base_types.kind = 'base_type'
                      and inherited_scopes.language = 'csharp'
                      and inherited_scopes.kind in ('using', 'namespace')
                      and c.caller like base_types.local_alias || '.%'
                      and c.callee like 'base.%'
                      and instr(substr(c.callee, length('base') + 2), '.') = 0
                      and (
                        s.name = substr(c.callee, length('base') + 2)
                        or s.qualified_name = substr(c.callee, length('base') + 2)
                        or s.qualified_name like '%.' || substr(c.callee, length('base') + 2)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        inherited_base_types.line as dependency_line,
                        s.start_line as start_line,
                        1 as match_rank
                    from calls c
                    join dependencies base_types
                      on base_types.source_file_id = c.source_file_id
                    join dependencies base_scopes
                      on base_scopes.source_file_id = c.source_file_id
                    join files base_files
                      on base_files.path like '%' ||
                        replace(base_scopes.target, '.', '/') ||
                        '/' ||
                        base_types.target ||
                        '.cs'
                    join dependencies inherited_base_types
                      on inherited_base_types.source_file_id = base_files.id
                    join dependencies inherited_scopes
                      on inherited_scopes.source_file_id = base_files.id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(inherited_scopes.target, '.', '/') ||
                        '/' ||
                        inherited_base_types.target ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and base_types.language = 'csharp'
                      and base_types.kind = 'base_type'
                      and base_types.local_alias is not null
                      and base_types.target not like '%.%'
                      and base_scopes.language = 'csharp'
                      and base_scopes.kind in ('using', 'namespace')
                      and inherited_base_types.language = 'csharp'
                      and inherited_base_types.kind = 'base_type'
                      and inherited_scopes.language = 'csharp'
                      and inherited_scopes.kind in ('using', 'namespace')
                      and c.caller like base_types.local_alias || '.%'
                      and c.callee like 'base.%'
                      and instr(substr(c.callee, length('base') + 2), '.') = 0
                      and (
                        s.name = substr(c.callee, length('base') + 2)
                        or s.qualified_name = substr(c.callee, length('base') + 2)
                        or s.qualified_name like '%.' || substr(c.callee, length('base') + 2)
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name,
                        property_types.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies property_types
                      on property_types.language = 'csharp'
                      and property_types.kind = 'property_type'
                      and property_types.local_alias is not null
                      and property_types.imported_symbol is not null
                    join files owner_files
                      on owner_files.id = property_types.source_file_id
                      and owner_files.path like '%' ||
                        replace(
                          substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          ),
                          '.',
                          '/'
                        ) ||
                        '.cs'
                    join dependencies property_scopes
                      on property_scopes.source_file_id = owner_files.id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(property_scopes.target, '.', '/') ||
                        '/' ||
                        property_types.target ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and property_scopes.language = 'csharp'
                      and property_scopes.kind in ('using', 'namespace')
                      and instr(c.callee, '.' || property_types.local_alias || '.') > 0
                      and (
                        property_types.imported_symbol =
                          substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          )
                        or substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          ) like '%.' || property_types.imported_symbol
                      )
                      and (
                        s.name = substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                        or s.qualified_name = substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                        or s.qualified_name like '%' || '.' || substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                      )

                    union all

                    select
                        c.id as call_id,
                        target_files.path as callee_file,
                        s.qualified_name,
                        property_types.line as dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from calls c
                    join dependencies property_types
                      on property_types.language = 'csharp'
                      and property_types.kind = 'property_type'
                      and property_types.local_alias is not null
                      and property_types.imported_symbol is not null
                    join files owner_files
                      on owner_files.id = property_types.source_file_id
                      and owner_files.path like '%' ||
                        replace(
                          substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          ),
                          '.',
                          '/'
                        ) ||
                        '.cs'
                    join files target_files
                      on target_files.path like '%' ||
                        replace(property_types.target, '.', '/') ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and property_types.target like '%.%'
                      and instr(c.callee, '.' || property_types.local_alias || '.') > 0
                      and (
                        property_types.imported_symbol =
                          substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          )
                        or substr(
                            c.callee,
                            1,
                            instr(c.callee, '.' || property_types.local_alias || '.') - 1
                          ) like '%.' || property_types.imported_symbol
                      )
                      and (
                        s.name = substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                        or s.qualified_name = substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                        or s.qualified_name like '%' || '.' || substr(
                          c.callee,
                          instr(c.callee, '.' || property_types.local_alias || '.') +
                            length(property_types.local_alias) + 2
                        )
                      )

                    union all

                    select
                        c.call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        c.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_type_bound_calls c
                    join dependencies scopes
                      on scopes.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(scopes.target, '.', '/') ||
                        '/' ||
                        c.target ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where scopes.language = 'csharp'
                      and scopes.kind in ('using', 'namespace')
                      and (
                        s.name = c.member_tail
                        or s.qualified_name = c.member_tail
                        or s.qualified_name like '%.' || c.member_tail
                      )

                    union all

                    select
                        p.call_id,
                        p.owner_file as callee_file,
                        s.qualified_name,
                        p.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_property_call_parts p
                    join symbols s on s.file_id = p.owner_file_id
                    where (
                        s.qualified_name =
                          p.property_type || '.' || p.member_tail
                        or s.qualified_name like
                          '%.' || p.property_type || '.' || p.member_tail
                      )

                    union all

                    select
                        p.call_id,
                        target_files.path as callee_file,
                        s.qualified_name,
                        p.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_property_call_parts p
                    join dependencies property_scopes
                      on property_scopes.source_file_id = p.owner_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(property_scopes.target, '.', '/') ||
                        '/' ||
                        p.property_type ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where property_scopes.language = 'csharp'
                      and property_scopes.kind in ('using', 'namespace')
                      and instr(p.member_tail, '.') = 0
                      and (
                        s.name = p.member_tail
                        or s.qualified_name = p.member_tail
                        or s.qualified_name like
                          '%.' || p.member_tail
                        or s.qualified_name =
                          p.property_type || '.' || p.member_tail
                        or s.qualified_name like
                          '%.' || p.property_type || '.' || p.member_tail
                      )

                    union all

                    select
                        p.call_id,
                        target_files.path as callee_file,
                        s.qualified_name,
                        p.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_property_call_parts p
                    join files target_files
                      on target_files.path like '%' ||
                        replace(p.property_type, '.', '/') ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where p.property_type like '%.%'
                      and instr(p.member_tail, '.') = 0
                      and (
                        s.name = p.member_tail
                        or s.qualified_name = p.member_tail
                        or s.qualified_name like
                          '%.' || p.member_tail
                        or s.qualified_name =
                          p.property_type || '.' || p.member_tail
                        or s.qualified_name like
                          '%.' || p.property_type || '.' || p.member_tail
                      )

                    union all

                    select
                        c.call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        c.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_type_bound_calls c
                    join files target_files
                      on target_files.path like '%' ||
                        replace(c.target, '.', '/') ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where c.target like '%.%'
                      and (
                        s.name = c.member_tail
                        or s.qualified_name = c.member_tail
                        or s.qualified_name like '%.' || c.member_tail
                      )

                    union all

                    select
                        c.call_id,
                        target_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        c.dependency_line,
                        s.start_line as start_line,
                        0 as match_rank
                    from temp.csharp_type_bound_calls c
                    join dependencies aliases
                      on aliases.source_file_id = c.source_file_id
                    join files target_files
                      on target_files.path like '%' ||
                        replace(aliases.target, '.', '/') ||
                        '.cs'
                    join symbols s on s.file_id = target_files.id
                    where aliases.language = 'csharp'
                      and aliases.kind = 'using_alias'
                      and aliases.local_alias = c.target
                      and (
                        s.name = c.member_tail
                        or s.qualified_name = c.member_tail
                        or s.qualified_name like '%.' || c.member_tail
                      )

                    union all

                    select
                        c.call_id,
                        extension_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        extension_methods.line as dependency_line,
                        s.start_line as start_line,
                        1 as match_rank
                    from temp.csharp_type_bound_calls c
                    join dependencies imported_extensions
                      on imported_extensions.source_file_id = c.source_file_id
                    join files extension_files
                      on extension_files.path = imported_extensions.resolved_file
                    join dependencies extension_methods
                      on extension_methods.source_file_id = extension_files.id
                    join symbols s
                      on s.file_id = extension_files.id
                    where imported_extensions.language = 'csharp'
                      and imported_extensions.kind in ('using', 'namespace')
                      and extension_methods.language = 'csharp'
                      and extension_methods.kind = 'extension_method'
                      and extension_methods.local_alias = c.member_tail
                      and (
                        extension_methods.target = c.target
                        or extension_methods.target like '%.' || c.target
                        or c.target like '%.' || extension_methods.target
                      )
                      and s.name = c.member_tail

                    union all

                    select
                        c.id as call_id,
                        extension_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        extension_methods.line as dependency_line,
                        s.start_line as start_line,
                        2 as match_rank
                    from calls c
                    join dependencies imported_extensions
                      on imported_extensions.source_file_id = c.source_file_id
                    join files extension_files
                      on extension_files.path = imported_extensions.resolved_file
                    join dependencies extension_methods
                      on extension_methods.source_file_id = extension_files.id
                    join symbols s
                      on s.file_id = extension_files.id
                    where c.callee_file is null
                      and c.language = 'csharp'
                      and c.callee like '%.%'
                      and imported_extensions.language = 'csharp'
                      and imported_extensions.kind in ('using', 'namespace')
                      and extension_methods.language = 'csharp'
                      and extension_methods.kind = 'extension_method'
                      and (
                        c.callee =
                          extension_methods.target ||
                          '.' ||
                          extension_methods.local_alias
                        or c.callee like
                          '%.' ||
                          extension_methods.target ||
                          '.' ||
                          extension_methods.local_alias
                      )
                      and s.name = extension_methods.local_alias

                    union all

                    select
                        c.id as call_id,
                        reexport_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        -1 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join dependencies rd on rd.source_file_id = target_files.id
                    join files reexport_files on reexport_files.path = rd.resolved_file
                    join symbols s on s.file_id = reexport_files.id
                    where c.callee_file is null
                      and rd.kind = 'export_alias'
                      and (
                        rd.local_alias = c.callee
                        or (
                            d.local_alias = c.callee
                            and d.imported_symbol is not null
                            and d.imported_symbol != '*'
                            and rd.local_alias = d.imported_symbol
                        )
                        or (
                            d.imported_symbol = '*'
                            and c.callee like d.local_alias || '.%'
                            and rd.local_alias = substr(c.callee, length(d.local_alias) + 2)
                        )
                      )
                      and (
                        s.name = rd.imported_symbol
                        or s.qualified_name = rd.imported_symbol
                        or s.qualified_name like '%.' || rd.imported_symbol
                      )

                    union all

                    select
                        c.id as call_id,
                        reexport_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        -1 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join dependencies rd on rd.source_file_id = target_files.id
                    join files reexport_files on reexport_files.path = rd.resolved_file
                    join symbols s on s.file_id = reexport_files.id
                    where c.callee_file is null
                      and rd.kind = 'export_namespace'
                      and (
                        (
                            rd.local_alias is null
                            and (
                                s.name = c.callee
                                or s.qualified_name = c.callee
                                or s.qualified_name like '%.' || c.callee
                                or (
                                    d.local_alias = c.callee
                                    and d.imported_symbol is not null
                                    and d.imported_symbol != '*'
                                    and (
                                        s.name = d.imported_symbol
                                        or s.qualified_name = d.imported_symbol
                                        or s.qualified_name like '%.' || d.imported_symbol
                                    )
                                )
                                or (
                                    d.imported_symbol = '*'
                                    and c.callee like d.local_alias || '.%'
                                    and (
                                        s.name = substr(c.callee, length(d.local_alias) + 2)
                                        or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                        or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                                    )
                                )
                            )
                        )
                        or (
                            rd.local_alias is not null
                            and (
                                (
                                    c.callee like rd.local_alias || '.%'
                                    and (
                                        s.name = substr(c.callee, length(rd.local_alias) + 2)
                                        or s.qualified_name = substr(c.callee, length(rd.local_alias) + 2)
                                        or s.qualified_name like '%.' || substr(c.callee, length(rd.local_alias) + 2)
                                    )
                                )
                                or (
                                    d.local_alias is not null
                                    and d.imported_symbol = rd.local_alias
                                    and c.callee like d.local_alias || '.%'
                                    and (
                                        s.name = substr(c.callee, length(d.local_alias) + 2)
                                        or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                        or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                                    )
                                )
                                or (
                                    d.imported_symbol = '*'
                                    and c.callee like d.local_alias || '.' || rd.local_alias || '.%'
                                    and (
                                        s.name = substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                                        or s.qualified_name = substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                                        or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                                    )
                                )
                            )
                        )
                      )

                    union all

                    select
                        c.id as call_id,
                        second_reexport_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        -2 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join dependencies rd on rd.source_file_id = target_files.id
                    join files first_reexport_files on first_reexport_files.path = rd.resolved_file
                    join dependencies rd2 on rd2.source_file_id = first_reexport_files.id
                    join files second_reexport_files on second_reexport_files.path = rd2.resolved_file
                    join symbols s on s.file_id = second_reexport_files.id
                    where c.callee_file is null
                      and rd.kind = 'export_alias'
                      and rd2.kind = 'export_alias'
                      and rd2.local_alias = rd.imported_symbol
                      and (
                        rd.local_alias = c.callee
                        or (
                            d.local_alias = c.callee
                            and d.imported_symbol is not null
                            and d.imported_symbol != '*'
                            and rd.local_alias = d.imported_symbol
                        )
                        or (
                            d.imported_symbol = '*'
                            and c.callee like d.local_alias || '.%'
                            and rd.local_alias = substr(c.callee, length(d.local_alias) + 2)
                        )
                      )
                      and (
                        s.name = rd2.imported_symbol
                        or s.qualified_name = rd2.imported_symbol
                        or s.qualified_name like '%.' || rd2.imported_symbol
                      )

                    union all

                    select
                        c.id as call_id,
                        second_reexport_files.path as callee_file,
                        s.qualified_name as qualified_name,
                        d.line as dependency_line,
                        s.start_line as start_line,
                        -2 as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join dependencies rd on rd.source_file_id = target_files.id
                    join files first_reexport_files on first_reexport_files.path = rd.resolved_file
                    join dependencies rd2 on rd2.source_file_id = first_reexport_files.id
                    join files second_reexport_files on second_reexport_files.path = rd2.resolved_file
                    join symbols s on s.file_id = second_reexport_files.id
                    where c.callee_file is null
                      and rd.kind = 'export_alias'
                      and rd2.kind = 'export_namespace'
                      and rd2.local_alias = rd.imported_symbol
                      and (
                        (
                            c.callee like rd.local_alias || '.%'
                            and (
                                s.name = substr(c.callee, length(rd.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(rd.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(rd.local_alias) + 2)
                            )
                        )
                        or (
                            d.local_alias is not null
                            and d.imported_symbol = rd.local_alias
                            and c.callee like d.local_alias || '.%'
                            and (
                                s.name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + 2)
                            )
                        )
                        or (
                            d.imported_symbol = '*'
                            and c.callee like d.local_alias || '.' || rd.local_alias || '.%'
                            and (
                                s.name = substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                                or s.qualified_name = substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                                or s.qualified_name like '%.' || substr(c.callee, length(d.local_alias) + length(rd.local_alias) + 3)
                            )
                        )
                      )
                )
            )
            where target_rank = 1;

            create unique index imported_call_targets_call_id
                on imported_call_targets(call_id);
            ",
        )?;

        let updated = self.conn.execute(
            "
            update calls
            set
                callee_file = (
                    select callee_file
                    from temp.imported_call_targets
                    where call_id = calls.id
                ),
                confidence = max(confidence, 0.72)
            where id in (
                select call_id from temp.imported_call_targets
            )
            ",
            [],
        )?;

        self.conn
            .execute("drop table if exists temp.imported_call_targets", [])?;
        self.conn
            .execute("drop table if exists temp.csharp_property_call_parts", [])?;

        Ok(updated + self.resolve_go_package_calls()?)
    }

    fn resolve_go_package_calls(&self) -> Result<usize> {
        let unresolved_calls = {
            let mut stmt = self.conn.prepare(
                "select distinct c.id, c.callee, d.local_alias, d.resolved_file
                 from calls c
                 join dependencies d on d.source_file_id = c.source_file_id
                 where c.callee_file is null
                   and c.language = 'go'
                   and d.language = 'go'
                   and d.local_alias is not null
                   and d.resolved_file is not null
                   and c.callee like d.local_alias || '.%'
                 order by c.id, d.line",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut targets = Vec::new();
        let mut symbol_stmt = self.conn.prepare(
            "select f.path
             from symbols s
             join files f on f.id = s.file_id
             where f.language = 'go'
               and s.kind = 'function'
               and s.name = ?1
             order by f.path, s.start_line",
        )?;
        for (call_id, callee, local_alias, resolved_file) in unresolved_calls {
            let Some(member) = callee.strip_prefix(&format!("{local_alias}.")) else {
                continue;
            };
            if member.is_empty() || member.contains('.') {
                continue;
            }
            let Some(package_dir) = Path::new(&resolved_file).parent() else {
                continue;
            };
            let rows = symbol_stmt.query_map(params![member], |row| row.get::<_, String>(0))?;
            let mut package_matches = rows
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .filter(|file| Path::new(file).parent() == Some(package_dir))
                .collect::<Vec<_>>();
            package_matches.sort();
            package_matches.dedup();
            if package_matches.len() == 1 {
                targets.push((call_id, package_matches.remove(0)));
            }
        }
        drop(symbol_stmt);

        let mut updated = 0;
        let mut update = self.conn.prepare(
            "update calls
             set callee_file = ?1, confidence = max(confidence, 0.72)
             where id = ?2 and callee_file is null",
        )?;
        for (call_id, callee_file) in targets {
            updated += update.execute(params![callee_file, call_id])?;
        }
        Ok(updated)
    }

    pub fn indexed_files(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("select path from files order by path")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            create table if not exists index_meta (
                key text primary key,
                value text not null
            );

            create table if not exists files (
                id integer primary key autoincrement,
                path text not null unique,
                language text not null,
                hash text not null,
                line_count integer not null,
                size integer,
                modified_ns integer
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

            create table if not exists dependencies (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                target text not null,
                resolved_file text,
                local_alias text,
                imported_symbol text,
                kind text not null,
                language text not null,
                line integer not null
            );

            create table if not exists calls (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                caller text not null,
                callee text not null,
                callee_file text,
                language text not null,
                line integer not null,
                column integer not null,
                confidence real not null
            );

            create table if not exists semantic_chunks (
                id integer primary key autoincrement,
                file_id integer not null references files(id) on delete cascade,
                start_line integer not null,
                end_line integer not null,
                content_hash text not null,
                token_estimate integer not null,
                text text not null,
                updated_at integer not null,
                unique(file_id, start_line, end_line)
            );

            create table if not exists semantic_embeddings (
                id integer primary key autoincrement,
                chunk_id integer not null references semantic_chunks(id) on delete cascade,
                provider text not null,
                model text not null,
                dimensions integer not null,
                vector blob not null,
                updated_at integer not null,
                unique(chunk_id, provider, model)
            );

            create index if not exists idx_symbols_name on symbols(name);
            create index if not exists idx_symbols_qualified_name on symbols(qualified_name);
            create index if not exists idx_symbols_file_name on symbols(file_id, name);
            create index if not exists idx_symbols_file_qualified_name on symbols(file_id, qualified_name);
            create index if not exists idx_dependencies_source on dependencies(source_file_id);
            create index if not exists idx_dependencies_target on dependencies(target);
            create index if not exists idx_calls_caller on calls(caller);
            create index if not exists idx_calls_callee on calls(callee);
            create index if not exists idx_calls_source_callee on calls(source_file_id, callee);
            create index if not exists idx_semantic_chunks_file on semantic_chunks(file_id);
            create index if not exists idx_semantic_chunks_hash on semantic_chunks(content_hash);
            create index if not exists idx_semantic_embeddings_provider_model
                on semantic_embeddings(provider, model);
            ",
        )?;
        self.ensure_column("dependencies", "resolved_file", "resolved_file text")?;
        self.ensure_column("files", "size", "size integer")?;
        self.ensure_column("files", "modified_ns", "modified_ns integer")?;
        self.ensure_column("dependencies", "local_alias", "local_alias text")?;
        self.ensure_column("dependencies", "imported_symbol", "imported_symbol text")?;
        self.ensure_column("calls", "callee_file", "callee_file text")?;
        self.conn.execute(
            "create index if not exists idx_dependencies_source_resolved_file
             on dependencies(source_file_id, resolved_file)",
            [],
        )?;
        self.conn.execute(
            "create index if not exists idx_dependencies_alias
             on dependencies(source_file_id, local_alias, imported_symbol, resolved_file)",
            [],
        )?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> Result<()> {
        let mut stmt = self.conn.prepare(&format!("pragma table_info({table})"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !columns.iter().any(|existing| existing == column) {
            self.conn
                .execute(&format!("alter table {table} add column {definition}"), [])?;
        }
        Ok(())
    }

    fn ensure_schema_version(&self) -> Result<()> {
        let version = self
            .get_meta("schema_version")?
            .and_then(|value| value.parse::<i64>().ok());
        if version != Some(SCHEMA_VERSION) {
            self.conn.execute("delete from semantic_embeddings", [])?;
            self.conn.execute("delete from semantic_chunks", [])?;
            self.conn.execute("delete from calls", [])?;
            self.conn.execute("delete from dependencies", [])?;
            self.conn.execute("delete from symbols", [])?;
            self.conn.execute("delete from files", [])?;
            self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
            self.set_meta("index_version", INDEX_VERSION)?;
        }
        Ok(())
    }

    fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("select value from index_meta where key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "insert into index_meta (key, value)
             values (?1, ?2)
             on conflict(key) do update set value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

pub fn cache_dir(root: &Path) -> PathBuf {
    root.join(".codeinsight")
}

fn semantic_chunk_from_row(row: &Row<'_>) -> rusqlite::Result<SemanticChunk> {
    Ok(SemanticChunk {
        id: row.get(0)?,
        file: row.get(1)?,
        start_line: row.get::<_, i64>(2)? as usize,
        end_line: row.get::<_, i64>(3)? as usize,
        token_estimate: row.get::<_, i64>(4)? as usize,
        text: row.get(5)?,
    })
}

fn encode_f32_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(vector));
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_f32_vector(bytes: &[u8], dimensions: usize) -> Result<Vec<f32>> {
    let expected_bytes = dimensions * std::mem::size_of::<f32>();
    if bytes.len() != expected_bytes {
        bail!(
            "semantic embedding vector has {} bytes, expected {}",
            bytes.len(),
            expected_bytes
        );
    }

    Ok(bytes
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
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
        "bash" => Language::Bash,
        "c" => Language::C,
        "cpp" => Language::Cpp,
        "csharp" => Language::CSharp,
        "go" => Language::Go,
        "java" => Language::Java,
        "php" => Language::Php,
        "python" => Language::Python,
        "ruby" => Language::Ruby,
        "rust" => Language::Rust,
        "typescript" => Language::TypeScript,
        "tsx" => Language::Tsx,
        _ => Language::JavaScript,
    }
}

fn dependency_graph_filter_clause(
    files: &[String],
    languages: &[String],
    kinds: &[String],
    params: &mut Vec<SqlValue>,
) -> String {
    let mut conditions = Vec::new();
    if !files.is_empty() {
        let placeholders = vec!["?"; files.len()].join(", ");
        conditions.push(format!(
            "(f.path in ({placeholders}) or d.resolved_file in ({placeholders}))"
        ));
        params.extend(files.iter().cloned().map(SqlValue::Text));
        params.extend(files.iter().cloned().map(SqlValue::Text));
    }
    if !languages.is_empty() {
        let placeholders = vec!["?"; languages.len()].join(", ");
        conditions.push(format!("d.language in ({placeholders})"));
        params.extend(languages.iter().cloned().map(SqlValue::Text));
    }
    if !kinds.is_empty() {
        let placeholders = vec!["?"; kinds.len()].join(", ");
        conditions.push(format!("d.kind in ({placeholders})"));
        params.extend(kinds.iter().cloned().map(SqlValue::Text));
    }

    if conditions.is_empty() {
        String::new()
    } else {
        format!("where {}", conditions.join(" and "))
    }
}

fn overview_summary(
    indexed_files: usize,
    total_lines: usize,
    symbols: usize,
    dependencies: usize,
    type_relation_edges: usize,
    call_edges: usize,
    languages: &[LanguageStat],
    top_directories: &[DirectoryStat],
) -> String {
    let primary_language = languages
        .first()
        .map(|language| language.language.as_str())
        .unwrap_or("none");
    let top_directory = top_directories
        .first()
        .map(|directory| directory.directory.as_str())
        .unwrap_or(".");
    format!(
        "{indexed_files} indexed files, {total_lines} lines, {symbols} symbols, {dependencies} dependency edges, {type_relation_edges} type-relation edges, {call_edges} call edges. Primary language: {primary_language}. Largest directory: {top_directory}."
    )
}

fn recommended_next_tools(
    root: &Path,
    entrypoints: &[EntryPointCandidate],
    dependency_summary: &DependencySummary,
    call_summary: &CallSummary,
) -> Vec<RecommendedToolCall> {
    let root = root.display().to_string();
    let mut tools = Vec::new();
    let source_entrypoint = entrypoints
        .iter()
        .find(|entrypoint| entrypoint.role == "source");

    tools.push(RecommendedToolCall {
        tool: "context_pack".to_string(),
        priority: 10,
        reason: if entrypoints.iter().any(|entrypoint| entrypoint.role == "source") {
            "Build first-read context from the highest-confidence source entrypoint.".to_string()
        } else {
            "Build first-read context from indexed source files because no source entrypoint was detected.".to_string()
        },
        suggested_arguments: json!({
            "root": root,
            "task": "understand project entrypoint and main flow",
            "token_budget": 6000
        }),
    });

    if dependency_summary.edges > 0 {
        let mut suggested_arguments = json!({
            "root": root,
            "limit": 100
        });
        let reason = if let Some(entrypoint) = source_entrypoint {
            suggested_arguments["files"] = json!([entrypoint.file.clone()]);
            format!(
                "Inspect dependency edges touching the source entrypoint {} before deeper navigation.",
                entrypoint.file
            )
        } else if let Some(target) = dependency_summary.top_external_targets.first() {
            format!(
                "Inspect module and package relationships; the most frequent external target is {}.",
                target.target
            )
        } else {
            "Inspect module and package relationships before deeper navigation.".to_string()
        };
        tools.push(RecommendedToolCall {
            tool: "dependency_graph".to_string(),
            priority: 30,
            reason,
            suggested_arguments,
        });
    }

    if dependency_summary.type_relation_edges > 0 {
        let relation_target = dependency_summary
            .top_type_relation_targets
            .first()
            .map(|target| target.target.as_str())
            .unwrap_or("type contracts");
        tools.push(RecommendedToolCall {
            tool: "dependency_graph".to_string(),
            priority: 25,
            reason: format!(
                "Inspect {} direct type-relation edges before editing inherited contracts; the most frequent relation target is {}.",
                dependency_summary.type_relation_edges, relation_target
            ),
            suggested_arguments: json!({
                "root": root,
                "kinds": ["base_type"],
                "limit": 100
            }),
        });
    }

    if let Some(entrypoint) = source_entrypoint {
        let mut suggested_arguments = json!({
            "root": root,
            "files": [entrypoint.file.clone()],
            "limit": 20,
            "depth": 2,
            "format": "summary",
            "evidence_limit": 5
        });
        if let Some(symbol) = &entrypoint.symbol {
            suggested_arguments["symbols"] = json!([symbol]);
        }
        tools.push(RecommendedToolCall {
            tool: "impact_analysis".to_string(),
            priority: 40,
            reason: "Estimate the entrypoint change radius using call and dependency signals."
                .to_string(),
            suggested_arguments,
        });
    } else if call_summary.edges > 0 {
        tools.push(RecommendedToolCall {
            tool: "callers".to_string(),
            priority: 40,
            reason: "Inspect static call graph edges because no source entrypoint was detected."
                .to_string(),
            suggested_arguments: json!({
                "root": root,
                "symbol": "<replace-with-symbol>",
                "limit": 50
            }),
        });
    }

    tools.push(RecommendedToolCall {
        tool: "config_status".to_string(),
        priority: 80,
        reason: "Check project-specific validation commands before planning changes.".to_string(),
        suggested_arguments: json!({
            "root": root
        }),
    });

    tools.sort_by_key(|tool| tool.priority);
    tools
}

fn file_entrypoint_signal(file: &str) -> Option<(usize, String)> {
    let basename = file.rsplit('/').next().unwrap_or(file).to_ascii_lowercase();
    let path = file.to_ascii_lowercase();
    if basename.starts_with("main.") {
        Some((100, "conventional main file".to_string()))
    } else if is_next_app_router_entrypoint(&path, &basename) {
        Some((82, "Next.js app router entrypoint".to_string()))
    } else if is_next_pages_entrypoint(&path, &basename) {
        Some((82, "Next.js pages bootstrap entrypoint".to_string()))
    } else if path == "config/routes.rb" || path.ends_with("/config/routes.rb") {
        Some((82, "Rails route entrypoint".to_string()))
    } else if is_python_web_entrypoint(&path, &basename) {
        Some((79, "Python web framework entrypoint".to_string()))
    } else if is_csharp_web_entrypoint(&basename) {
        Some((79, "C# web application entrypoint".to_string()))
    } else if basename == "lib.rs" {
        Some((90, "Rust library root".to_string()))
    } else if basename == "mod.rs" {
        Some((85, "Rust module root".to_string()))
    } else if basename.starts_with("index.") {
        Some((80, "conventional index file".to_string()))
    } else if basename.starts_with("app.") {
        Some((75, "conventional app file".to_string()))
    } else if path.contains("/server.") || basename.starts_with("server.") {
        Some((70, "server entrypoint naming".to_string()))
    } else if path.contains("/cli.") || basename.starts_with("cli.") {
        Some((65, "CLI entrypoint naming".to_string()))
    } else if basename.ends_with("application.java") {
        Some((62, "Java application entrypoint naming".to_string()))
    } else {
        None
    }
}

fn is_next_app_router_entrypoint(path: &str, basename: &str) -> bool {
    (path.starts_with("app/") || path.contains("/app/"))
        && (basename.starts_with("page.")
            || basename.starts_with("layout.")
            || basename.starts_with("route."))
}

fn is_next_pages_entrypoint(path: &str, basename: &str) -> bool {
    (path.starts_with("pages/") || path.contains("/pages/"))
        && (basename.starts_with("_app.") || basename.starts_with("_document."))
}

fn is_python_web_entrypoint(path: &str, basename: &str) -> bool {
    basename == "manage.py"
        || basename == "asgi.py"
        || basename == "wsgi.py"
        || basename == "urls.py"
        || path.ends_with("/manage.py")
        || path.ends_with("/asgi.py")
        || path.ends_with("/wsgi.py")
        || path.ends_with("/urls.py")
}

fn is_csharp_web_entrypoint(basename: &str) -> bool {
    basename == "program.cs" || basename == "startup.cs"
}

fn path_role(path: &str) -> &'static str {
    let normalized = path.to_ascii_lowercase();
    if normalized == "node_modules" || normalized.starts_with("node_modules/") {
        "vendor"
    } else if normalized.contains("fixture") || normalized.contains("fixtures/") {
        "fixture"
    } else if normalized == "test"
        || normalized == "tests"
        || normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.ends_with("_test")
        || normalized.ends_with("_tests")
    {
        "test"
    } else if normalized == "examples" || normalized.starts_with("examples/") {
        "example"
    } else if normalized == "docs" || normalized.starts_with("docs/") {
        "docs"
    } else {
        "source"
    }
}

fn entrypoint_confidence(score: usize) -> f64 {
    ((score.min(110) as f64) / 110.0 * 100.0).round() / 100.0
}

fn entrypoint_sort_score(candidate: &EntryPointCandidate) -> i32 {
    candidate.score as i32 + entrypoint_path_priority(&candidate.file)
}

fn entrypoint_path_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        -40
    } else if matches!(path_role(file), "docs" | "test" | "fixture" | "vendor") {
        -60
    } else {
        0
    }
}

fn symbol_entrypoint_signal(symbol: &str) -> Option<(usize, String)> {
    match symbol.to_ascii_lowercase().as_str() {
        "main" => Some((110, "entry symbol named main".to_string())),
        "run" | "start" => Some((88, format!("entry-like symbol named {symbol}"))),
        "server" | "handler" => Some((78, format!("service entry-like symbol named {symbol}"))),
        _ => None,
    }
}

fn upsert_entrypoint_candidate(
    candidates: &mut BTreeMap<String, EntryPointCandidate>,
    file: &str,
    language: &str,
    score: usize,
    reason: String,
    symbol: Option<String>,
) {
    let candidate = candidates
        .entry(file.to_string())
        .or_insert_with(|| EntryPointCandidate {
            file: file.to_string(),
            language: language.to_string(),
            role: path_role(file).to_string(),
            score,
            confidence: entrypoint_confidence(score),
            reason: reason.clone(),
            symbol: symbol.clone(),
        });
    if score > candidate.score {
        candidate.score = score;
        candidate.confidence = entrypoint_confidence(score);
        candidate.reason = reason;
        candidate.symbol = symbol;
    } else if candidate.symbol.is_none() && symbol.is_some() {
        candidate.symbol = symbol;
    }
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_semantic_chunks_preserves_unchanged_embeddings() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut store = Store::open(temp.path())?;
        store.upsert_file(&SourceFile {
            path: temp.path().join("src/main.rs"),
            relative_path: "src/main.rs".to_string(),
            language: Language::Rust,
            hash: "file-hash".to_string(),
            line_count: 2,
        })?;
        let chunk = SemanticChunkInput {
            file: "src/main.rs".to_string(),
            start_line: 1,
            end_line: 2,
            content_hash: "chunk-a".to_string(),
            token_estimate: 2,
            text: "fn main() {}".to_string(),
        };

        let first_stats = store.replace_semantic_chunks(std::slice::from_ref(&chunk), true)?;
        assert_eq!(first_stats.total, 1);
        assert_eq!(first_stats.added, 1);
        assert_eq!(first_stats.updated, 0);
        assert_eq!(first_stats.removed, 0);
        assert_eq!(first_stats.changes.len(), 1);
        assert_eq!(first_stats.changes[0].change, "added");
        assert_eq!(first_stats.changes[0].file, "src/main.rs");
        assert_eq!(first_stats.changes[0].start_line, 1);
        assert_eq!(first_stats.changes[0].end_line, 2);
        assert_eq!(first_stats.changes[0].previous_hash, None);
        assert_eq!(
            first_stats.changes[0].content_hash.as_deref(),
            Some("chunk-a")
        );
        let chunks = store.semantic_chunks()?;
        store.upsert_semantic_embeddings(
            "local-hash",
            "local-hash-v1",
            &[SemanticEmbeddingInput {
                chunk_id: chunks[0].id,
                vector: vec![0.1, 0.2],
            }],
        )?;

        let unchanged_stats = store.replace_semantic_chunks(std::slice::from_ref(&chunk), true)?;
        assert_eq!(unchanged_stats.total, 1);
        assert_eq!(unchanged_stats.added, 0);
        assert_eq!(unchanged_stats.updated, 0);
        assert_eq!(unchanged_stats.removed, 0);
        assert!(unchanged_stats.changes.is_empty());

        assert_eq!(
            store.count_semantic_embeddings_for("local-hash", "local-hash-v1")?,
            1
        );
        assert!(
            store
                .semantic_chunks_missing_embeddings("local-hash", "local-hash-v1")?
                .is_empty()
        );

        let changed_chunk = SemanticChunkInput {
            content_hash: "chunk-b".to_string(),
            text: "fn main() { println!(\"changed\"); }".to_string(),
            ..chunk
        };

        let changed_stats =
            store.replace_semantic_chunks(std::slice::from_ref(&changed_chunk), true)?;
        assert_eq!(changed_stats.total, 1);
        assert_eq!(changed_stats.added, 0);
        assert_eq!(changed_stats.updated, 1);
        assert_eq!(changed_stats.removed, 0);
        assert_eq!(changed_stats.changes.len(), 1);
        assert_eq!(changed_stats.changes[0].change, "updated");
        assert_eq!(
            changed_stats.changes[0].previous_hash.as_deref(),
            Some("chunk-a")
        );
        assert_eq!(
            changed_stats.changes[0].content_hash.as_deref(),
            Some("chunk-b")
        );

        assert_eq!(
            store.count_semantic_embeddings_for("local-hash", "local-hash-v1")?,
            0
        );
        assert_eq!(
            store
                .semantic_chunks_missing_embeddings("local-hash", "local-hash-v1")?
                .len(),
            1
        );

        let removed_stats = store.replace_semantic_chunks(&[], true)?;
        assert_eq!(removed_stats.total, 0);
        assert_eq!(removed_stats.added, 0);
        assert_eq!(removed_stats.updated, 0);
        assert_eq!(removed_stats.removed, 1);
        assert_eq!(removed_stats.changes.len(), 1);
        assert_eq!(removed_stats.changes[0].change, "removed");
        assert_eq!(
            removed_stats.changes[0].previous_hash.as_deref(),
            Some("chunk-b")
        );
        assert_eq!(removed_stats.changes[0].content_hash, None);

        Ok(())
    }

    #[test]
    fn migrates_missing_resolved_file_column_even_when_meta_is_current() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cache = cache_dir(temp.path());
        std::fs::create_dir_all(&cache)?;
        let conn = Connection::open(cache.join("index.db"))?;
        conn.execute_batch(
            "
            create table index_meta (
                key text primary key,
                value text not null
            );
            insert into index_meta (key, value) values ('schema_version', '3');

            create table files (
                id integer primary key autoincrement,
                path text not null unique,
                language text not null,
                hash text not null,
                line_count integer not null
            );

            create table symbols (
                id integer primary key autoincrement,
                file_id integer not null references files(id) on delete cascade,
                name text not null,
                qualified_name text not null,
                kind text not null,
                language text not null,
                start_line integer not null,
                end_line integer not null
            );

            create table dependencies (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                target text not null,
                kind text not null,
                language text not null,
                line integer not null
            );

            create table calls (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                caller text not null,
                callee text not null,
                language text not null,
                line integer not null,
                column integer not null,
                confidence real not null
            );
            ",
        )?;
        drop(conn);

        let mut store = Store::open(temp.path())?;
        let file_id = store.upsert_file(&SourceFile {
            path: temp.path().join("src/main.rs"),
            relative_path: "src/main.rs".to_string(),
            language: Language::Rust,
            hash: "hash".to_string(),
            line_count: 1,
        })?;
        assert_eq!(
            store.file_index_metadata("src/main.rs")?,
            Some(FileIndexMetadata {
                hash: "hash".to_string(),
                size: None,
                modified_ns: None,
            })
        );
        store.replace_dependencies(
            file_id,
            &[Dependency {
                source_file: "src/main.rs".to_string(),
                target: "crate::lib".to_string(),
                resolved_file: Some("src/lib.rs".to_string()),
                local_alias: None,
                imported_symbol: None,
                kind: "use".to_string(),
                language: Language::Rust,
                line: 1,
            }],
        )?;

        let graph = store.dependency_graph(temp.path(), 10, 0, &[], &[], &[])?;
        assert_eq!(graph.dependencies.len(), 1);
        assert_eq!(
            graph.dependencies[0].resolved_file.as_deref(),
            Some("src/lib.rs")
        );

        Ok(())
    }

    #[test]
    fn migrates_missing_call_callee_file_column_even_when_meta_is_current() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cache = cache_dir(temp.path());
        std::fs::create_dir_all(&cache)?;
        let conn = Connection::open(cache.join("index.db"))?;
        conn.execute_batch(
            "
            create table index_meta (
                key text primary key,
                value text not null
            );
            insert into index_meta (key, value) values ('schema_version', '21');

            create table files (
                id integer primary key autoincrement,
                path text not null unique,
                language text not null,
                hash text not null,
                line_count integer not null
            );

            create table symbols (
                id integer primary key autoincrement,
                file_id integer not null references files(id) on delete cascade,
                name text not null,
                qualified_name text not null,
                kind text not null,
                language text not null,
                start_line integer not null,
                end_line integer not null
            );

            create table dependencies (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                target text not null,
                resolved_file text,
                kind text not null,
                language text not null,
                line integer not null
            );

            create table calls (
                id integer primary key autoincrement,
                source_file_id integer not null references files(id) on delete cascade,
                caller text not null,
                callee text not null,
                language text not null,
                line integer not null,
                column integer not null,
                confidence real not null
            );
            ",
        )?;
        drop(conn);

        let mut store = Store::open(temp.path())?;
        let file_id = store.upsert_file(&SourceFile {
            path: temp.path().join("src/main.ts"),
            relative_path: "src/main.ts".to_string(),
            language: Language::TypeScript,
            hash: "hash".to_string(),
            line_count: 1,
        })?;
        store.replace_calls(
            file_id,
            &[CallEdge {
                file: "src/main.ts".to_string(),
                caller: "main".to_string(),
                callee: "render".to_string(),
                callee_file: Some("src/ui.ts".to_string()),
                language: Language::TypeScript,
                line: 1,
                column: 1,
                confidence: 0.72,
            }],
        )?;

        let calls = store.callees("main", 10)?;
        assert_eq!(calls[0].callee_file.as_deref(), Some("src/ui.ts"));

        Ok(())
    }
}
