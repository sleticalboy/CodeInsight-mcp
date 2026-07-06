use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::model::{
    CallEdge, Dependency, DependencyGraph, DirectoryStat, Language, LanguageStat, ProjectOverview,
    SourceFile, Symbol, SymbolKind,
};

pub const SCHEMA_VERSION: i64 = 21;
pub const INDEX_VERSION: &str = env!("CARGO_PKG_VERSION");

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
        store.ensure_schema_version()?;
        Ok(store)
    }

    pub fn reset(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("delete from calls", [])?;
        tx.execute("delete from dependencies", [])?;
        tx.execute("delete from symbols", [])?;
        tx.execute("delete from files", [])?;
        tx.commit()?;
        Ok(())
    }

    pub fn mark_indexed(&self) -> Result<()> {
        self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
        self.set_meta("index_version", INDEX_VERSION)?;
        self.set_meta("last_indexed_at", &unix_timestamp().to_string())?;
        Ok(())
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

    pub fn file_hash(&self, relative_path: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("select hash from files where path = ?1")?;
        let mut rows = stmt.query(params![relative_path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
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

    pub fn dependency_graph(&self, root: &Path, limit: usize) -> Result<DependencyGraph> {
        let mut stmt = self.conn.prepare(
            "select f.path, d.target, d.resolved_file, d.local_alias, d.imported_symbol, d.kind, d.language, d.line
             from dependencies d
             join files f on f.id = d.source_file_id
             order by f.path, d.line
             limit ?1",
        )?;
        let dependencies = stmt
            .query_map(params![limit as i64], |row| {
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

        Ok(DependencyGraph {
            root: root.display().to_string(),
            dependencies,
            nodes,
            edges,
        })
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
                            when c.callee like 'require.%'
                              and d.line = c.line
                              and (
                                s.name = substr(c.callee, length('require') + 2)
                                or s.qualified_name = substr(c.callee, length('require') + 2)
                                or s.qualified_name like '%.' || substr(c.callee, length('require') + 2)
                              )
                                then 0
                            when s.name = c.callee then 1
                            else 2
                        end as match_rank
                    from calls c
                    join dependencies d on d.source_file_id = c.source_file_id
                    join files target_files on target_files.path = d.resolved_file
                    join symbols s on s.file_id = target_files.id
                    where c.callee_file is null
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

            create index if not exists idx_symbols_name on symbols(name);
            create index if not exists idx_symbols_qualified_name on symbols(qualified_name);
            create index if not exists idx_symbols_file_name on symbols(file_id, name);
            create index if not exists idx_symbols_file_qualified_name on symbols(file_id, qualified_name);
            create index if not exists idx_dependencies_source on dependencies(source_file_id);
            create index if not exists idx_dependencies_target on dependencies(target);
            create index if not exists idx_calls_caller on calls(caller);
            create index if not exists idx_calls_callee on calls(callee);
            create index if not exists idx_calls_source_callee on calls(source_file_id, callee);
            ",
        )?;
        self.ensure_column("dependencies", "resolved_file", "resolved_file text")?;
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
        "c" => Language::C,
        "cpp" => Language::Cpp,
        "csharp" => Language::CSharp,
        "go" => Language::Go,
        "java" => Language::Java,
        "python" => Language::Python,
        "rust" => Language::Rust,
        "typescript" => Language::TypeScript,
        "tsx" => Language::Tsx,
        _ => Language::JavaScript,
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

        let graph = store.dependency_graph(temp.path(), 10)?;
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
