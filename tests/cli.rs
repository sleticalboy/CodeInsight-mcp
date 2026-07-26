use std::io::Write;
use std::path::Path;

use assert_cmd::Command;
use predicates::str::{contains, is_match};
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn cli_indexes_and_queries_fixture_project() {
    let fixture = fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 31);
    assert_eq!(index["changed_files"], 31);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let overview = run_json(["overview", fixture.path().to_str().unwrap()]);
    assert_eq!(overview["indexed_files"], 31);
    assert!(overview["total_lines"].as_u64().unwrap() > 0);
    assert!(
        overview["summary"]
            .as_str()
            .unwrap()
            .contains("indexed files")
    );
    assert!(
        overview["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|language| language["language"] == "typescript")
    );
    assert!(overview["main_directories"].as_array().unwrap().iter().any(
        |directory| directory["directory"] == "src"
            && directory["role"] == "source"
            && directory["files"].as_u64().unwrap() > 0
            && directory["symbols"].as_u64().unwrap() > 0
    ));
    assert!(
        overview["symbol_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind["kind"] == "function" && kind["symbols"].as_u64().unwrap() > 0)
    );
    assert!(
        overview["dependency_summary"]["edges"].as_u64().unwrap()
            >= overview["dependency_summary"]["resolved_edges"]
                .as_u64()
                .unwrap()
    );
    assert!(
        overview["dependency_summary"]["external_targets"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        !overview["dependency_summary"]["top_external_targets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        overview["call_summary"]["resolved_callee_edges"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        overview["entrypoints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entrypoint| entrypoint["file"] == "src/main.ts"
                && entrypoint["symbol"] == "main"
                && entrypoint["role"] == "source"
                && entrypoint["confidence"].as_f64().unwrap() >= 1.0
                && entrypoint["reason"]
                    .as_str()
                    .unwrap()
                    .contains("entry symbol"))
    );
    assert_eq!(
        overview["index_status"]["index_version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(
        overview["index_status"]["last_indexed_at"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        overview["recommended_next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "context_pack"
                && tool["priority"].as_u64() == Some(10)
                && tool["suggested_arguments"]["task"]
                    == "understand project entrypoint and main flow"
                && tool["suggested_arguments"]["token_budget"].as_u64() == Some(6000))
    );
    assert!(
        overview["recommended_next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "dependency_graph"
                && tool["priority"].as_u64() == Some(30)
                && tool["suggested_arguments"]["limit"].as_u64() == Some(100)
                && tool["suggested_arguments"]["files"][0] == "src/main.ts"
                && tool["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("source entrypoint")))
    );
    assert!(
        overview["recommended_next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["tool"] == "impact_analysis"
                && tool["priority"].as_u64() == Some(40)
                && tool["suggested_arguments"]["files"][0] == "src/main.ts"
                && tool["suggested_arguments"]["symbols"][0] == "main")
    );

    let second_index = run_json(["index", fixture.path().to_str().unwrap()]);
    assert_eq!(second_index["changed_files"], 0);
    assert_eq!(second_index["unchanged_files"], 31);

    let semantic_index = run_json([
        "semantic-index",
        fixture.path().to_str().unwrap(),
        "--chunk-lines",
        "20",
    ]);
    assert_eq!(
        semantic_index["vector_status"],
        "chunks_indexed_without_embeddings"
    );
    let semantic_chunks = semantic_index["chunks"].as_u64().unwrap();
    assert!(semantic_chunks > 0);
    assert_eq!(
        semantic_index["chunks_added"].as_u64(),
        Some(semantic_chunks)
    );
    assert_eq!(semantic_index["chunks_updated"].as_u64(), Some(0));
    assert_eq!(semantic_index["chunks_removed"].as_u64(), Some(0));
    assert_eq!(semantic_index["embeddings"].as_u64(), Some(0));
    assert_eq!(semantic_index["embeddings_generated"].as_u64(), Some(0));
    assert_eq!(semantic_index["embeddings_reused"].as_u64(), Some(0));

    let embedding_status = run_json(["embedding-status", fixture.path().to_str().unwrap()]);
    assert_eq!(embedding_status["provider"], "disabled");
    assert_eq!(embedding_status["configured"], false);
    assert_eq!(embedding_status["source"], "default");
    assert_eq!(embedding_status["batch_size"].as_u64(), Some(64));
    assert_eq!(
        embedding_status["batch_size_env"],
        "CODEINSIGHT_EMBEDDING_BATCH_SIZE"
    );
    assert_eq!(
        embedding_status["index"]["vector_status"],
        "provider_not_configured"
    );
    assert_eq!(
        embedding_status["index"]["chunks"].as_u64(),
        Some(semantic_chunks)
    );
    let context_without_ollama_vectors = run_json_with_env(
        [
            "context-pack",
            fixture.path().to_str().unwrap(),
            "--task",
            "session cookie behavior",
            "--symbol",
            "AuthService",
            "--token-budget",
            "1600",
        ],
        [
            ("CODEINSIGHT_EMBEDDING_PROVIDER", "ollama"),
            ("CODEINSIGHT_OLLAMA_BASE_URL", "http://127.0.0.1:9"),
        ],
    );
    assert!(
        context_without_ollama_vectors["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/auth_notes.py")
    );
    assert_eq!(
        context_without_ollama_vectors["semantic_status"]["vector_status"],
        "embeddings_missing_for_provider"
    );
    assert!(
        context_without_ollama_vectors["semantic_status"]["fallback_candidates"]
            .as_u64()
            .unwrap()
            > 0
    );

    let semantic_index_with_embeddings = run_json_with_env(
        [
            "semantic-index",
            fixture.path().to_str().unwrap(),
            "--chunk-lines",
            "20",
        ],
        [("CODEINSIGHT_EMBEDDING_PROVIDER", "local-hash")],
    );
    assert_eq!(semantic_index_with_embeddings["provider"], "local-hash");
    assert_eq!(
        semantic_index_with_embeddings["vector_status"],
        "embeddings_indexed"
    );
    assert_eq!(
        semantic_index_with_embeddings["embeddings"].as_u64(),
        Some(semantic_chunks)
    );
    assert_eq!(
        semantic_index_with_embeddings["chunks_added"].as_u64(),
        Some(0)
    );
    assert_eq!(
        semantic_index_with_embeddings["chunks_updated"].as_u64(),
        Some(0)
    );
    assert_eq!(
        semantic_index_with_embeddings["chunks_removed"].as_u64(),
        Some(0)
    );
    assert_eq!(
        semantic_index_with_embeddings["embeddings_generated"].as_u64(),
        Some(semantic_chunks)
    );
    assert_eq!(
        semantic_index_with_embeddings["embeddings_reused"].as_u64(),
        Some(0)
    );
    let repeated_semantic_index_with_embeddings = run_json_with_env(
        [
            "semantic-index",
            fixture.path().to_str().unwrap(),
            "--chunk-lines",
            "20",
        ],
        [("CODEINSIGHT_EMBEDDING_PROVIDER", "local-hash")],
    );
    assert_eq!(
        repeated_semantic_index_with_embeddings["embeddings_generated"].as_u64(),
        Some(0)
    );
    assert_eq!(
        repeated_semantic_index_with_embeddings["embeddings_reused"].as_u64(),
        Some(semantic_chunks)
    );
    let embedding_status_with_provider = run_json_with_env(
        ["embedding-status", fixture.path().to_str().unwrap()],
        [("CODEINSIGHT_EMBEDDING_PROVIDER", "local-hash")],
    );
    assert_eq!(embedding_status_with_provider["provider"], "local-hash");
    assert_eq!(embedding_status_with_provider["model"], "local-hash-v1");
    assert_eq!(embedding_status_with_provider["configured"], true);
    assert_eq!(
        embedding_status_with_provider["index"]["vector_status"],
        "embeddings_indexed"
    );
    assert_eq!(
        embedding_status_with_provider["index"]["embeddings"].as_u64(),
        Some(semantic_chunks)
    );
    let semantic_search = run_json_with_env(
        [
            "semantic-search",
            fixture.path().to_str().unwrap(),
            "session cookie behavior",
            "--limit",
            "5",
        ],
        [("CODEINSIGHT_EMBEDDING_PROVIDER", "local-hash")],
    );
    assert!(semantic_search.as_array().unwrap().iter().any(|result| {
        result["file"] == "src/auth_notes.py"
            && result["excerpt"].as_str().unwrap().contains("cookie")
            && result["score"].as_f64().unwrap() > 0.0
    }));
    let vector_context = run_json_with_env(
        [
            "context-pack",
            fixture.path().to_str().unwrap(),
            "--task",
            "session cookie behavior",
            "--symbol",
            "AuthService",
            "--token-budget",
            "1600",
        ],
        [("CODEINSIGHT_EMBEDDING_PROVIDER", "local-hash")],
    );
    let vector_context_file = vector_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/auth_notes.py")
        .unwrap();
    assert!(
        vector_context_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| range["reason"]
                .as_str()
                .unwrap()
                .contains("Semantic vector match"))
    );
    assert_eq!(
        vector_context["semantic_status"]["vector_status"],
        "vector_matches_available"
    );
    assert!(
        vector_context["semantic_status"]["selected_vector_ranges"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(vector_context["semantic_status"]["provider"], "local-hash");

    let symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "AuthService",
        "--limit",
        "5",
    ]);
    assert_eq!(symbols[0]["name"], "AuthService");

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "200",
    ]);
    let targets = deps["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|dependency| dependency["target"].as_str())
        .collect::<Vec<_>>();
    assert!(targets.contains(&"os"));
    assert!(targets.contains(&"./ui"));
    assert!(targets.contains(&"@app/path-ui"));
    assert!(targets.contains(&"@app/special"));
    assert!(targets.contains(&"@app/special/button"));
    assert!(targets.contains(&"@fallback/fallback-ui"));
    assert!(targets.contains(&"@base/base-ui"));
    assert!(targets.contains(&"shared"));
    assert!(targets.contains(&"fixture-lib/package-ui"));
    assert!(targets.contains(&"dep-lib/feature"));
    assert!(targets.contains(&"dep-lib/array-feature"));
    assert!(targets.contains(&"dep-lib/node-feature"));
    assert!(targets.contains(&"browser-lib"));
    assert!(targets.contains(&"browser-external-lib"));
    assert!(targets.contains(&"browser-object-lib/server"));
    assert!(targets.contains(&"browser-object-lib/plain"));
    assert!(targets.contains(&"browser-object-lib/external"));
    assert!(targets.contains(&"browser-object-lib/absolute"));
    assert!(targets.contains(&"browser-object-lib/object"));
    assert!(targets.contains(&"browser-object-lib/disabled"));
    assert!(targets.contains(&"legacy-lib"));
    assert!(targets.contains(&"root-array-lib"));
    assert!(targets.contains(&"root-browser-export-lib"));
    assert!(targets.contains(&"metadata-invalid-lib"));
    assert!(targets.contains(&"legacy-lib/plugin"));
    assert!(targets.contains(&"workspace-ui/button"));
    assert!(targets.contains(&"#internal/logger"));
    assert!(targets.contains(&"#internal/special"));
    assert!(targets.contains(&"#internal/special/button"));
    assert!(targets.contains(&"#fallback/logger"));
    assert!(targets.contains(&"@multi/admin/component/card"));
    assert!(targets.contains(&"fixture-lib/multi/admin/component/card"));
    assert!(targets.contains(&"#multi/admin/component/card"));
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "./ui" && dependency["resolved_file"] == "src/ui.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@base/base-ui"
                    && dependency["resolved_file"] == "src/base/base-ui.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "legacy-lib/plugin"
                    && dependency["resolved_file"] == "node_modules/legacy-lib/plugin/index.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "legacy-lib"
                    && dependency["resolved_file"] == "node_modules/legacy-lib/dist/index.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "root-array-lib"
                    && dependency["resolved_file"] == "node_modules/root-array-lib/dist/index.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "root-array-lib"
                    || dependency["resolved_file"] != "node_modules/root-array-lib/external-root.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "root-browser-export-lib"
                    && dependency["resolved_file"]
                        == "node_modules/root-browser-export-lib/dist/browser.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "root-browser-export-lib"
                    || dependency["resolved_file"]
                        != "node_modules/root-browser-export-lib/dist/node.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "metadata-invalid-lib"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "metadata-invalid-lib"
                    || dependency["resolved_file"]
                        != "node_modules/metadata-invalid-lib/external-entry.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "metadata-invalid-lib"
                    || dependency["resolved_file"]
                        != "node_modules/metadata-invalid-lib/dist/absolute-entry.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@fallback/fallback-ui"
                    && dependency["resolved_file"] == "src/fallback-ui.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "shared"
                    && dependency["resolved_file"] == "src/shared/index.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "dep-lib/node-feature"
                    && dependency["resolved_file"] == "node_modules/dep-lib/dist/node-feature.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "dep-lib/feature"
                    && dependency["resolved_file"] == "node_modules/dep-lib/dist/feature.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "dep-lib/array-feature"
                    && dependency["resolved_file"] == "node_modules/dep-lib/dist/array-feature.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-lib"
                    && dependency["resolved_file"]
                        == "node_modules/browser-lib/dist/browser-entry.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-external-lib"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-external-lib"
                    || dependency["resolved_file"]
                        != "node_modules/browser-external-lib/external-browser-entry.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-external-lib"
                    || dependency["resolved_file"]
                        != "node_modules/browser-external-lib/dist/node-entry.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/server"
                    && dependency["resolved_file"]
                        == "node_modules/browser-object-lib/dist/browser-server.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/plain"
                    && dependency["resolved_file"]
                        == "node_modules/browser-object-lib/dist/browser-plain.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/external"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/external"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/external-browser-shim.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/external"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/external.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/absolute"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/absolute"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/browser-absolute.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/absolute"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/absolute.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/object"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/object"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/browser-object.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/object"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/object.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "browser-object-lib/disabled"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "browser-object-lib/disabled"
                    || dependency["resolved_file"]
                        != "node_modules/browser-object-lib/dist/disabled.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "fixture-lib/package-ui"
                    && dependency["resolved_file"] == "src/package-ui.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "workspace-ui/button"
                    && dependency["resolved_file"] == "packages/workspace-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#internal/logger"
                    && dependency["resolved_file"] == "src/internal/logger.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#internal/special"
                    && dependency["resolved_file"] == "src/import-special.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#internal/special/button"
                    && dependency["resolved_file"] == "src/import-special/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#internal/special"
                    || dependency["resolved_file"] != "src/internal/special.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@multi/admin/component/card"
                    && dependency["resolved_file"] == "src/multi/admin/component/card.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "fixture-lib/multi/admin/component/card"
                    && dependency["resolved_file"] == "src/multi/admin/component/card.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#multi/admin/component/card"
                    && dependency["resolved_file"] == "src/multi/admin/component/card.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#fallback/logger"
                    && dependency["resolved_file"] == "src/internal/logger.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@app/path-ui"
                    && dependency["resolved_file"] == "src/path-ui.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@app/special"
                    && dependency["resolved_file"] == "src/path-special.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "@app/special/button"
                    && dependency["resolved_file"] == "src/path-special/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "@app/special"
                    || dependency["resolved_file"] != "src/special.ts"
            })
    );
    assert_eq!(deps["offset"].as_u64(), Some(0));
    assert_eq!(
        deps["page_size"].as_u64(),
        Some(deps["dependencies"].as_array().unwrap().len() as u64)
    );

    let dep_page = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "2",
        "--offset",
        "1",
    ]);
    assert_eq!(dep_page["edges"], deps["edges"]);
    assert_eq!(dep_page["summary"]["edges"], deps["summary"]["edges"]);
    assert_eq!(dep_page["limit"].as_u64(), Some(2));
    assert_eq!(dep_page["offset"].as_u64(), Some(1));
    assert_eq!(dep_page["page_size"].as_u64(), Some(2));
    assert_eq!(dep_page["has_more"].as_bool(), Some(true));
    assert_eq!(
        dep_page["dependencies"][0]["source_file"],
        deps["dependencies"][1]["source_file"]
    );
    assert_eq!(
        dep_page["dependencies"][0]["target"],
        deps["dependencies"][1]["target"]
    );

    let refs = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "AuthService",
        "--include-definitions",
    ]);
    assert!(refs.as_array().unwrap().iter().any(|reference| {
        reference["file"] == "src/auth.py" && reference["reference_kind"] == "definition"
    }));

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand auth flow",
        "--symbol",
        "AuthService",
        "--token-budget",
        "1600",
    ]);
    let selected_range_count = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["ranges"].as_array().unwrap().len() as u64)
        .sum::<u64>();
    assert_eq!(context["symbols"][0]["name"], "AuthService");
    assert_eq!(
        context["budget"]["requested_token_budget"].as_u64(),
        Some(1600)
    );
    assert_eq!(
        context["budget"]["applied_token_budget"].as_u64(),
        Some(1600)
    );
    assert_eq!(
        context["budget"]["estimated_tokens"],
        context["estimated_tokens"]
    );
    assert_eq!(context["budget"]["truncated"], context["truncated"]);
    assert_eq!(
        context["budget"]["selected_files"].as_u64(),
        Some(context["files"].as_array().unwrap().len() as u64)
    );
    assert_eq!(
        context["budget"]["selected_ranges"].as_u64(),
        Some(selected_range_count)
    );
    assert!(
        context["budget"]["candidate_files"].as_u64().unwrap()
            >= context["budget"]["selected_files"].as_u64().unwrap()
    );
    assert_eq!(
        context["budget"]["omitted_files"].as_u64().unwrap(),
        context["budget"]["candidate_files"].as_u64().unwrap()
            - context["budget"]["selected_files"].as_u64().unwrap()
    );
    assert_eq!(
        context["budget"]["omitted_ranges"].as_u64().unwrap(),
        context["budget"]["candidate_ranges"].as_u64().unwrap()
            - context["budget"]["selected_ranges"].as_u64().unwrap()
    );
    assert!(
        !context["budget"]["truncation_reason"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(
        !context["continuation_summary"]["status"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(
        !context["continuation_summary"]["message"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(
        !context["continuation_summary"]["next_action"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let omitted_candidates = context["omitted_candidates"].as_array().unwrap();
    if context["budget"]["omitted_files"].as_u64().unwrap() > 0 {
        assert!(!omitted_candidates.is_empty());
        assert!(
            omitted_candidates.len()
                <= context["budget"]["omitted_files"].as_u64().unwrap() as usize
        );
        let first_omitted = &omitted_candidates[0];
        assert!(first_omitted["file"].as_str().unwrap().starts_with("src/"));
        assert!(first_omitted["score"].as_i64().unwrap() > 0);
        assert!(first_omitted["selection_rank"].as_u64().unwrap() > 0);
        assert!(
            !first_omitted["omission_reason"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            first_omitted["next_action"],
            "run_omitted_candidate_context_pack"
        );
        assert!(!first_omitted["source"].as_str().unwrap().is_empty());
        assert!(!first_omitted["reason"].as_str().unwrap().is_empty());
        assert!(!first_omitted["ranges"].as_array().unwrap().is_empty());
        assert!(first_omitted["ranges"][0].get("excerpt").is_none());
        assert_eq!(first_omitted["suggested_tool"]["tool"], "context_pack");
        assert_eq!(
            first_omitted["suggested_tool"]["suggested_arguments"]["files"][0],
            first_omitted["file"]
        );
        assert_eq!(
            first_omitted["suggested_tool"]["suggested_arguments"]["token_budget"].as_u64(),
            Some(4000)
        );
        assert_eq!(
            context["continuation_summary"]["status"],
            "omitted_candidates_available"
        );
        assert_eq!(
            context["continuation_summary"]["omitted_candidate_count"].as_u64(),
            Some(omitted_candidates.len() as u64)
        );
        assert_eq!(
            context["continuation_summary"]["first_omitted_file"],
            first_omitted["file"]
        );
        assert_eq!(
            context["continuation_summary"]["suggested_tool"]["suggested_arguments"]["files"][0],
            first_omitted["file"]
        );
    } else {
        assert_eq!(
            context["continuation_summary"]["omitted_candidate_count"].as_u64(),
            Some(0)
        );
    }
    assert_eq!(context["seed_strategy"], "explicit");
    assert!(
        context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["kind"] == "symbol"
                && seed["value"] == "AuthService"
                && seed["source"] == "explicit")
    );
    assert!(
        context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["file"] == "src/auth.py" })
    );
    let auth_context_files = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(auth_context_files.first(), Some(&"src/auth.py"));
    assert!(auth_context_files.contains(&"src/consumer.py"));
    assert_eq!(context["reading_plan"][0]["order"].as_u64(), Some(1));
    assert_eq!(context["reading_plan"][0]["file"], "src/auth.py");
    assert_eq!(
        context["reading_plan"][0]["selection_rank"].as_u64(),
        Some(1)
    );
    assert_eq!(context["files"][0]["selection_rank"].as_u64(), Some(1));
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("symbol")
    );
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("authentication")
    );
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("session")
    );
    assert_eq!(
        context["reading_plan"][0]["next_action"],
        "inspect_symbol_definition"
    );
    let symbol_question = context["reading_plan"][0]["question"].as_str().unwrap();
    assert!(symbol_question.contains("authentication decisions"));
    assert!(symbol_question.contains("session boundaries"));
    assert!(symbol_question.contains("definition"));
    assert!(
        context["reading_plan"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("Read this step to answer:")
    );
    assert!(
        context["reading_plan"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("If deeper evidence is needed, call file_outline.")
    );
    assert!(
        context["reading_plan"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("Selection reason:")
    );
    assert_eq!(
        context["reading_plan"][0]["reason"]
            .as_str()
            .unwrap()
            .matches("evidence mix")
            .count(),
        1,
        "reading-plan reason should not duplicate evidence mix"
    );
    assert!(
        context["reading_plan"][0]["selection_reason"]
            .as_str()
            .unwrap()
            .contains("symbol")
    );
    assert_eq!(
        context["reading_plan"][0]["suggested_tool"]["tool"],
        "file_outline"
    );
    assert_eq!(
        context["reading_plan"][0]["suggested_tool"]["priority"].as_u64(),
        Some(10)
    );
    assert!(
        context["reading_plan"][0]["suggested_tool"]["suggested_arguments"]["path"]
            .as_str()
            .unwrap()
            .ends_with("src/auth.py")
    );
    assert!(
        context["reading_plan"][0]["ranges"][0]["start_line"]
            .as_u64()
            .unwrap()
            > 0
    );

    let auto_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
    ]);
    assert!(
        auto_context["summary"]
            .as_str()
            .unwrap()
            .contains("auto-selected seed files")
    );
    assert_eq!(auto_context["seed_strategy"], "auto_entrypoint");
    assert!(
        auto_context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["kind"] == "file"
                && seed["value"] == "src/main.ts"
                && seed["source"] == "overview_entrypoint"
                && seed["role"] == "source")
    );
    assert!(
        auto_context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/main.ts" && file["source"] == "seed_file")
    );

    let task_path_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "inspect src/auth.py before editing login behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(task_path_context["seed_strategy"], "auto_task_path");
    assert_eq!(
        task_path_context["selected_seeds"][0]["value"],
        "src/auth.py"
    );
    assert_eq!(
        task_path_context["selected_seeds"][0]["source"],
        "task_path"
    );
    assert_eq!(task_path_context["files"][0]["file"], "src/auth.py");
    assert_eq!(task_path_context["files"][0]["source"], "seed_file");

    let semantic_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "session cookie behavior",
        "--symbol",
        "AuthService",
        "--token-budget",
        "1600",
    ]);
    let semantic_file = semantic_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/auth_notes.py")
        .unwrap();
    assert!(
        semantic_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| range["reason"]
                .as_str()
                .unwrap()
                .contains("Semantic chunk match"))
    );
    assert_eq!(
        semantic_context["semantic_status"]["vector_status"],
        "provider_not_configured"
    );
    assert!(
        semantic_context["semantic_status"]["selected_fallback_ranges"]
            .as_u64()
            .unwrap()
            > 0
    );
    let semantic_step = semantic_context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/auth_notes.py")
        .unwrap();
    assert_eq!(semantic_step["next_action"], "review_semantic_matches");
    let semantic_question = semantic_step["question"].as_str().unwrap();
    assert!(semantic_question.contains("cookie"));
    assert!(semantic_question.contains("session"));
    assert_eq!(semantic_step["suggested_tool"]["tool"], "context_pack");
    assert_eq!(
        semantic_step["suggested_tool"]["suggested_arguments"]["task"],
        "session cookie behavior"
    );
    assert_eq!(
        semantic_step["suggested_tool"]["suggested_arguments"]["files"][0],
        "src/auth_notes.py"
    );
    assert_eq!(
        semantic_step["suggested_tool"]["suggested_arguments"]["token_budget"].as_u64(),
        Some(4000)
    );

    let billing_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand billing payment behavior",
        "--symbol",
        "Service",
        "--token-budget",
        "1600",
    ]);
    let billing_context_files = billing_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(billing_context_files.first(), Some(&"src/billing.py"));

    let main_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand ui flow",
        "--symbol",
        "main",
        "--token-budget",
        "1600",
    ]);
    assert!(
        main_context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["file"] == "src/ui.ts" })
    );
    let file_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand ui entry file",
        "--file",
        "src/main.ts",
        "--token-budget",
        "2000",
    ]);
    let context_files = file_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(context_files.first(), Some(&"src/main.ts"));
    assert!(context_files.contains(&"src/ui.ts"));

    let call_graph_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand call graph target",
        "--symbol",
        "callGraphEntry",
        "--token-budget",
        "1600",
    ]);
    let call_graph_context_files = call_graph_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert!(call_graph_context_files.contains(&"src/call-entry.ts"));
    assert!(call_graph_context_files.contains(&"src/barrel.ts"));
    assert!(call_graph_context_files.contains(&"src/ui.ts"));

    let seed_file_call_graph_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand call graph target",
        "--file",
        "src/call-entry.ts",
        "--token-budget",
        "1600",
    ]);
    let seed_file_call_graph_context_files = seed_file_call_graph_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert!(seed_file_call_graph_context_files.contains(&"src/call-entry.ts"));
    assert!(seed_file_call_graph_context_files.contains(&"src/barrel.ts"));
    assert!(seed_file_call_graph_context_files.contains(&"src/ui.ts"));

    let seed_file_caller_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand render callers",
        "--file",
        "src/ui.ts",
        "--token-budget",
        "1600",
    ]);
    let seed_file_caller_context_files = seed_file_caller_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(seed_file_caller_context_files.first(), Some(&"src/ui.ts"));
    assert!(seed_file_caller_context_files.contains(&"src/ui.ts"));
    assert!(seed_file_caller_context_files.contains(&"src/main.ts"));
    let seed_file_caller_file = seed_file_caller_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/ui.ts")
        .unwrap();
    assert_context_file_has_no_duplicate_lines(seed_file_caller_file);
    assert_context_file_ranges_have_reasons(seed_file_caller_file);

    let long_file_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand late entrypoint",
        "--file",
        "src/long.ts",
        "--token-budget",
        "1600",
    ]);
    let long_file = &long_file_context["files"].as_array().unwrap()[0];
    assert_eq!(long_file["file"], "src/long.ts");
    let long_excerpt = long_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(long_excerpt.contains("lateEntry"));
    assert!(!long_excerpt.contains("filler_60"));

    let small_budget_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand late entrypoint",
        "--file",
        "src/long.ts",
        "--token-budget",
        "500",
    ]);
    let small_budget_file = &small_budget_context["files"].as_array().unwrap()[0];
    assert_eq!(small_budget_file["file"], "src/long.ts");
    let small_budget_excerpt = small_budget_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(small_budget_excerpt.contains("import { render }"));
    assert!(small_budget_excerpt.contains("lateEntry"));
    assert!(!small_budget_excerpt.contains("filler_60"));

    let huge_seed_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand huge entrypoint",
        "--file",
        "src/huge.ts",
        "--token-budget",
        "500",
    ]);
    let huge_seed_file = &huge_seed_context["files"].as_array().unwrap()[0];
    assert_eq!(huge_seed_file["file"], "src/huge.ts");
    let huge_seed_excerpt = huge_seed_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(huge_seed_excerpt.contains("hugeEntry"));
    assert!(!huge_seed_excerpt.contains("huge_filler_80"));

    let multi_seed_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "targetLater behavior",
        "--file",
        "src/multi-long.ts",
        "--token-budget",
        "500",
    ]);
    let multi_seed_file = &multi_seed_context["files"].as_array().unwrap()[0];
    assert_eq!(multi_seed_file["file"], "src/multi-long.ts");
    let multi_seed_excerpt = multi_seed_file["ranges"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|range| range["excerpt"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(multi_seed_excerpt.contains("targetLater"));
    assert!(!multi_seed_excerpt.contains("unrelated_filler_40"));

    let multi_readable_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "targetLater behavior",
        "--file",
        "src/multi-long.ts",
        "--token-budget",
        "1600",
    ]);
    let multi_readable_file = &multi_readable_context["files"].as_array().unwrap()[0];
    assert_eq!(multi_readable_file["file"], "src/multi-long.ts");
    assert_context_file_ranges_are_sorted(multi_readable_file);

    let callers = run_json([
        "callers",
        fixture.path().to_str().unwrap(),
        "helper",
        "--limit",
        "5",
    ]);
    assert_eq!(callers[0]["caller"], "AuthService.login");
    assert_eq!(callers[0]["callee"], "helper");

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "helper",
        "--file",
        "src/auth.py",
        "--limit",
        "10",
    ]);
    assert_eq!(impact["format"], "full");
    assert_eq!(impact["evidence_limit"].as_u64(), Some(20));
    assert!(["low", "medium", "high"].contains(&impact["risk_level"].as_str().unwrap()));
    assert!(impact["impact_counts"]["impacted_files"].as_u64().unwrap() >= 1);
    assert!(!impact["top_reasons"].as_array().unwrap().is_empty());
    assert_eq!(impact["seed_symbols"][0], "helper");
    assert_eq!(impact["seed_files"][0], "src/auth.py");
    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/auth.py")
    );
    assert!(
        impact["callers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["caller"] == "AuthService.login" && call["callee"] == "helper")
    );
    assert!(
        impact["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| reference["file"] == "src/auth.py")
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthService.login",
        "--limit",
        "5",
    ]);
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["callee"] == "helper")
    );

    let imported_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "main",
        "--limit",
        "5",
    ]);
    assert!(
        imported_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "render" && call["callee_file"] == "src/ui.ts" })
    );

    let aliased_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "aliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        aliased_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "draw" && call["callee_file"] == "src/ui.ts" })
    );

    let namespace_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "namespaceMain",
        "--limit",
        "5",
    ]);
    assert!(
        namespace_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "ui.render" && call["callee_file"] == "src/ui.ts" })
    );

    let module_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "moduleAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        module_alias_callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == "uiModule.render" && call["callee_file"] == "src/ui.ts"
        })
    );

    let computed_module_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "computedModuleAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        computed_module_alias_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "computedUiModule.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let variable_module_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "variableModuleAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        variable_module_alias_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "modalModule.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "defaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "drawDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "reexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "relayRender" && call["callee_file"] == "src/ui.ts" })
    );

    let reexport_default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "reexportDefaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        reexport_default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "relayDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let export_star_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "exportStarMain",
        "--limit",
        "5",
    ]);
    assert!(
        export_star_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "starRender" && call["callee_file"] == "src/ui.ts" })
    );

    let namespace_reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "namespaceReexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        namespace_reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "uiApi.render" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_reexport_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopReexportMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_reexport_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "finalRender" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_default_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopDefaultMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_default_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["callee"] == "finalDefault" && call["callee_file"] == "src/ui.ts" })
    );

    let two_hop_namespace_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "twoHopNamespaceMain",
        "--limit",
        "5",
    ]);
    assert!(
        two_hop_namespace_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "finalApi.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let require_member_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "requireMemberMain",
        "--limit",
        "5",
    ]);
    assert!(
        require_member_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "require.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let computed_require_member_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "computedRequireMemberMain",
        "--limit",
        "5",
    ]);
    assert!(
        computed_require_member_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "require.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let variable_require_member_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "variableRequireMemberMain",
        "--limit",
        "5",
    ]);
    assert!(
        variable_require_member_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "require.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let variable_template_require_member_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "variableTemplateRequireMemberMain",
        "--limit",
        "5",
    ]);
    assert!(
        variable_template_require_member_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "require.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let dynamic_import_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "dynamicImportMain",
        "--limit",
        "5",
    ]);
    assert!(
        dynamic_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "loadedUi.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let variable_dynamic_import_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "variableDynamicImportMain",
        "--limit",
        "5",
    ]);
    assert!(
        variable_dynamic_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "modalLoaded.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let dynamic_import_callback_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "import.then.<callback>",
        "--limit",
        "5",
    ]);
    assert!(
        dynamic_import_callback_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "thenUi.render" && call["callee_file"] == "src/ui.ts"
            })
    );

    let path_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "pathAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        path_alias_callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == "pathRender" && call["callee_file"] == "src/path-ui.ts"
        })
    );

    let path_alias_precedence_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "pathAliasPrecedenceMain",
        "--limit",
        "6",
    ]);
    assert!(
        path_alias_precedence_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "specialPathRender"
                    && call["callee_file"] == "src/path-special.ts"
            })
    );
    assert!(
        path_alias_precedence_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "specialButtonPathRender"
                    && call["callee_file"] == "src/path-special/button.ts"
            })
    );

    let fallback_alias_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "fallbackAliasMain",
        "--limit",
        "5",
    ]);
    assert!(
        fallback_alias_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "fallbackRender" && call["callee_file"] == "src/fallback-ui.ts"
            })
    );

    let base_url_index_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "baseUrlIndexMain",
        "--limit",
        "5",
    ]);
    assert!(
        base_url_index_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "sharedRender" && call["callee_file"] == "src/shared/index.ts"
            })
    );

    let inherited_paths_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "inheritedPathsMain",
        "--limit",
        "5",
    ]);
    assert!(
        inherited_paths_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "baseRender" && call["callee_file"] == "src/base/base-ui.ts"
            })
    );

    let package_export_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "packageExportMain",
        "--limit",
        "5",
    ]);
    assert!(
        package_export_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "packageRender" && call["callee_file"] == "src/package-ui.ts"
            })
    );

    let workspace_package_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "workspacePackageMain",
        "--limit",
        "5",
    ]);
    assert!(
        workspace_package_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "workspaceButton"
                    && call["callee_file"] == "packages/workspace-ui/src/button.ts"
            })
    );

    let package_import_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "packageImportMain",
        "--limit",
        "5",
    ]);
    assert!(
        package_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "logInternal" && call["callee_file"] == "src/internal/logger.ts"
            })
    );
    assert!(
        package_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "specialInternalRender"
                    && call["callee_file"] == "src/import-special.ts"
            })
    );
    assert!(
        package_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "specialInternalButtonRender"
                    && call["callee_file"] == "src/import-special/button.ts"
            })
    );

    let dependency_package_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "dependencyPackageMain",
        "--limit",
        "10",
    ]);
    assert!(
        dependency_package_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| {
                call["callee"] != "browserDisabledRender"
                    || call["callee_file"] != "node_modules/browser-object-lib/dist/disabled.js"
            })
    );
}

#[test]
fn cli_agent_route_runs_first_read_pipeline() {
    let fixture = fixture_project();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--impact-limit",
        "10",
        "--impact-depth",
        "2",
        "--impact-evidence-limit",
        "3",
    ]);

    assert_eq!(route["task"], "understand app entrypoint flow");
    assert_eq!(route["token_budget"].as_u64(), Some(1600));
    assert_eq!(route["index_report"]["indexed_files"].as_u64(), Some(31));
    assert_eq!(route["overview"]["indexed_files"].as_u64(), Some(31));
    assert_eq!(route["context_pack"]["seed_strategy"], "auto_entrypoint");
    assert_eq!(route["context_pack"]["files"][0]["file"], "src/main.ts");
    assert_eq!(
        route["context_pack"]["reading_plan"][0]["file"],
        "src/main.ts"
    );
    assert_eq!(route["routing_decision"]["route_quality"]["level"], "high");
    assert!(
        route["routing_decision"]["route_quality"]["score"]
            .as_u64()
            .unwrap()
            >= 80
    );
    assert!(
        route["routing_decision"]["route_quality"]["evidence_count"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_selected_context"
    );
    assert!(
        route["routing_decision"]["route_quality"]["decision_summary"]
            .as_str()
            .unwrap()
            .contains("Read src/main.ts first with high confidence")
    );
    assert!(
        route["routing_decision"]["route_quality"]["decision_summary"]
            .as_str()
            .unwrap()
            .contains("candidate rank 1")
    );
    assert!(
        route["routing_decision"]["route_quality"]["evidence_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source == "seed file")
    );
    assert!(
        route["routing_decision"]["route_quality"]["confidence_factors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|factor| factor == "first selected file is candidate rank 1")
    );
    assert!(
        route["routing_decision"]["route_quality"]["verification_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str().unwrap().contains("Review impact_analysis"))
    );
    assert_eq!(route["impact_status"], "complete");
    assert_eq!(route["impact_analysis"]["format"], "summary");
    assert_eq!(route["impact_analysis"]["depth"].as_u64(), Some(2));
    assert_eq!(route["impact_analysis"]["evidence_limit"].as_u64(), Some(3));
    let context_reason = route["route"][2]["reason"].as_str().unwrap();
    assert!(context_reason.contains("read src/main.ts first"));
    assert!(context_reason.contains("candidate rank 1"));
    assert!(context_reason.contains("inspect_seed_file"));
    assert!(context_reason.contains("file_outline"));
    assert!(context_reason.contains("continuation"));
    let execution_actions = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        execution_actions,
        vec![
            "read_selected_context",
            "use_current_reading_step_suggested_tool",
            "use_continuation_if_needed",
            "review_impact_before_edits"
        ]
    );
    assert_eq!(route["execution_plan"][0]["status"], "ready");
    assert_eq!(route["execution_plan"][0]["files"][0], "src/main.ts");
    assert!(
        route["execution_plan"][0]["instruction"]
            .as_str()
            .unwrap()
            .contains("reading_plan[] order")
    );
    assert_agent_route_execution_plan_matches_context(&route);
    assert_eq!(
        route["execution_plan"][1]["suggested_tool"]["tool"],
        "file_outline"
    );
    assert_eq!(route["execution_plan"][2]["status"], "complete");
    assert_eq!(route["execution_plan"][3]["status"], "complete");
    let impact_reason = route["route"][3]["reason"].as_str().unwrap();
    assert!(impact_reason.contains("pre-edit impact check"));
    assert!(impact_reason.contains("call-related files"));
    assert!(impact_reason.contains("dependency-related files"));
    assert!(impact_reason.contains("call paths"));
    assert!(impact_reason.contains("dependency paths"));
    assert!(
        route["impact_seed_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file == "src/main.ts")
    );
    let tools = route["route"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["tool"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        tools,
        vec![
            "index_project",
            "project_overview",
            "context_pack",
            "impact_analysis"
        ]
    );
}

#[test]
fn cli_agent_route_accepts_backend_evidence_file() {
    let fixture = fixture_project();
    let evidence_path = fixture.path().join("backend-evidence.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/main.ts", "src/server.ts"],
            "evidence_sources": ["entry_points", "call_graph"],
            "evidence_count": 7,
            "latency_ms": 42,
            "confidence": 0.91,
            "notes": ["external graph backend agreed with local first-read route"]
        })
        .to_string(),
    )
    .unwrap();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence",
        evidence_path.to_str().unwrap(),
    ]);

    assert_eq!(
        route["routing_decision"]["backend_evidence"]["provider"],
        "codebase-memory-mcp"
    );
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["candidate_files"][0],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "agree"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["provider"],
        "codebase-memory-mcp"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["local_first_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["recommended_action"],
        "read_selected_context"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["common_files"][0],
        "src/main.ts"
    );
    assert!(
        route["routing_decision"]["route_quality"]["evidence_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source == "backend:codebase-memory-mcp:call_graph")
    );
    assert!(
        route["routing_decision"]["route_quality"]["confidence_factors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|factor| factor.as_str().unwrap().contains(
                "backend codebase-memory-mcp independently selected the same first file"
            ))
    );
    assert!(
        route["routing_decision"]["route_quality"]["verification_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .unwrap()
                .contains("Treat backend codebase-memory-mcp evidence as advisory"))
    );
}

#[test]
fn cli_agent_route_accepts_inline_backend_evidence_json() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidate_files": ["src/main.ts", "src/server.ts"],
        "evidence_sources": ["search_graph"],
        "evidence_count": 5,
        "confidence": 0.9
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    assert_eq!(
        route["routing_decision"]["backend_evidence"]["provider"],
        "codebase-memory-mcp"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "agree"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_selected_context"
    );
    assert!(
        route["routing_decision"]["route_quality"]["evidence_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source == "backend:codebase-memory-mcp:search_graph")
    );
}

#[test]
fn cli_agent_route_normalizes_inline_backend_tool_results() {
    let fixture = fixture_project();
    let local_route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
    ]);
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "tool_results": {
            "search_graph": {
                "elapsed_ms": 7,
                "results": [
                    {
                        "name": "main",
                        "label": "Function",
                        "file_path": fixture.path().join("src/main.ts"),
                        "score": 0.97
                    },
                    {
                        "name": "startServer",
                        "label": "Function",
                        "file_path": "src/server.ts"
                    }
                ]
            },
            "search_code": {
                "structuredContent": {
                    "duration_ms": 23,
                    "results": [
                        {
                            "node": "main",
                            "label": "Function",
                            "file": "src/main.ts"
                        },
                        {
                            "node": "startServer",
                            "label": "Function",
                            "file": "src/server.ts"
                        }
                    ]
                }
            },
            "get_architecture": {
                "content": [{
                    "type": "text",
                    "text": serde_json::json!({
                        "elapsed_ms": 3,
                        "entry_points": [{
                            "name": "render",
                            "file": "src/ui.ts",
                            "confidence": 0.88
                        }]
                    }).to_string()
                }]
            }
        }
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(
        evidence["candidate_files"],
        serde_json::json!(["src/main.ts", "src/server.ts", "src/ui.ts"])
    );
    assert_eq!(evidence["candidates"][0]["symbol"], "main");
    assert_eq!(evidence["candidates"][0]["source"], "search_graph");
    assert_eq!(evidence["candidates"][0]["reason"], "search_graph Function");
    assert_eq!(evidence["evidence_count"], 5);
    assert_eq!(
        route["routing_decision"]["route_quality"]["evidence_count"]
            .as_u64()
            .unwrap(),
        local_route["routing_decision"]["route_quality"]["evidence_count"]
            .as_u64()
            .unwrap()
            + 5
    );
    assert_eq!(evidence["latency_ms"], 33);
    assert_eq!(
        evidence["evidence_sources"],
        serde_json::json!([
            "search_graph",
            "search_code",
            "get_architecture:entry_points"
        ])
    );
    assert!(evidence.get("tool_results").is_none());
    assert!(
        evidence["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note == "normalized from inline backend tool_results")
    );
}

#[test]
fn cli_agent_route_bounds_inline_backend_tool_results() {
    let fixture = fixture_project();
    let results = (1..=70)
        .map(|rank| {
            serde_json::json!({
                "name": format!("candidate{rank}"),
                "label": "Function",
                "file_path": format!("src/raw-candidate-{rank}.ts")
            })
        })
        .collect::<Vec<_>>();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "tool_results": {
            "search_graph": {
                "results": results
            }
        }
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(evidence["candidate_files"].as_array().unwrap().len(), 16);
    assert_eq!(evidence["candidates"].as_array().unwrap().len(), 16);
    assert_eq!(evidence["evidence_count"], 64);
    assert_eq!(evidence["normalization"]["omitted_tool_result_items"], 6);
    assert_eq!(evidence["normalization"]["omitted_candidates"], 48);
    assert!(
        route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("omitted 6 raw tool result item(s), 48 candidate(s)"))
    );
}

#[test]
fn cli_agent_route_preserves_structured_backend_candidates() {
    let fixture = fixture_project();
    let absolute_main = fixture.path().join("src/main.ts");
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidate_files": ["src/server.ts", "src/main.ts"],
        "candidates": [
            {
                "file": absolute_main,
                "symbol": " main ",
                "source": " search_graph ",
                "score": 0.97,
                "reason": " entrypoint owns startup ",
                "evidence": [" definition ", "definition", " inbound_calls "]
            }
        ]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(
        evidence["candidate_files"],
        serde_json::json!(["src/main.ts", "src/server.ts"])
    );
    assert_eq!(evidence["candidates"][0]["file"], "src/main.ts");
    assert_eq!(evidence["candidates"][0]["symbol"], "main");
    assert_eq!(evidence["candidates"][0]["source"], "search_graph");
    assert_eq!(
        evidence["candidates"][0]["evidence"],
        serde_json::json!(["definition", "inbound_calls"])
    );
    assert_eq!(
        route["routing_decision"]["backend_selected_candidate"]["reason"],
        "entrypoint owns startup"
    );
    assert!(
        route["routing_decision"]["route_quality"]["evidence_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source == "backend:codebase-memory-mcp:candidate_evidence:inbound_calls")
    );
}

#[test]
fn cli_agent_route_help_documents_backend_evidence_stdin() {
    Command::cargo_bin("codeinsight")
        .unwrap()
        .args(["agent-route", "--help"])
        .assert()
        .success()
        .stdout(contains("--backend-evidence <PATH>"))
        .stdout(contains("--backend-evidence-json <JSON_OR_DASH>"))
        .stdout(contains("use '-' to read stdin"))
        .stdout(contains("--backend-fallback"))
        .stdout(contains("--prefer-backend-context"));
}

#[test]
fn cli_agent_route_accepts_backend_evidence_json_from_stdin() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidate_files": ["src/main.ts", "src/server.ts"],
        "evidence_sources": ["search_graph"],
        "evidence_count": 5,
        "confidence": 0.9
    })
    .to_string();

    let output = Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--token-budget",
            "1600",
            "--force-index",
            "--backend-evidence-json",
            "-",
        ])
        .write_stdin(backend_evidence)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let route: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(
        route["routing_decision"]["backend_evidence"]["provider"],
        "codebase-memory-mcp"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "agree"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_selected_context"
    );
}

#[test]
fn cli_agent_route_rejects_invalid_backend_evidence_json_from_stdin() {
    let fixture = fixture_project();

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            "-",
        ])
        .write_stdin("not-json")
        .assert()
        .failure()
        .stderr(contains("failed to parse inline backend evidence JSON"));
}

#[test]
fn cli_agent_route_normalizes_backend_evidence_before_routing() {
    let fixture = fixture_project();
    let absolute_main = fixture.path().join("src/main.ts");
    let backend_evidence = serde_json::json!({
        "provider": "  codebase-memory-mcp  ",
        "candidate_files": [absolute_main, "src/main.ts", "src/server.ts"],
        "evidence_sources": [" search_graph ", "search_graph", ""],
        "evidence_count": 5,
        "confidence": 0.9,
        "notes": [" verified route ", "verified route"]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(evidence["provider"], "codebase-memory-mcp");
    assert_eq!(
        evidence["candidate_files"],
        serde_json::json!(["src/main.ts", "src/server.ts"])
    );
    assert_eq!(
        evidence["evidence_sources"],
        serde_json::json!(["search_graph"])
    );
    assert_eq!(evidence["notes"], serde_json::json!(["verified route"]));
    assert!(evidence.get("candidates").is_none());
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "agree"
    );
}

#[test]
fn cli_agent_route_bounds_backend_evidence_for_token_safety() {
    let fixture = fixture_project();
    let candidates = (0..20)
        .map(|index| {
            serde_json::json!({
                "file": if index == 0 {
                    "src/main.ts".to_string()
                } else {
                    format!("graph/candidate-{index}.ts")
                },
                "symbol": (index == 0).then(|| "s".repeat(200)),
                "source": (index == 0).then(|| "q".repeat(200)),
                "reason": (index == 0).then(|| "r".repeat(400)),
                "evidence": if index < 5 {
                    (0..if index == 0 { 10 } else { 6 })
                        .map(|item| {
                            if index == 0 && item == 0 {
                                "e".repeat(200)
                            } else {
                                format!("evidence-{index}-{item}")
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            })
        })
        .collect::<Vec<_>>();
    let evidence_sources = (0..16)
        .map(|index| {
            if index == 0 {
                "source".repeat(40)
            } else {
                format!("source-{index}")
            }
        })
        .collect::<Vec<_>>();
    let notes = (0..10)
        .map(|index| {
            if index == 0 {
                "note".repeat(100)
            } else {
                format!("note-{index}")
            }
        })
        .collect::<Vec<_>>();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidates": candidates,
        "evidence_sources": evidence_sources,
        "notes": notes
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(evidence["candidate_files"].as_array().unwrap().len(), 16);
    assert_eq!(evidence["candidates"].as_array().unwrap().len(), 16);
    assert_eq!(
        evidence["candidates"][0]["symbol"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        160
    );
    assert_eq!(
        evidence["candidates"][0]["source"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        160
    );
    assert_eq!(
        evidence["candidates"][0]["reason"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        320
    );
    assert_eq!(
        evidence["candidates"][0]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        evidence["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| {
                candidate["evidence"]
                    .as_array()
                    .map_or(0, |evidence| evidence.len())
            })
            .sum::<usize>(),
        24
    );
    assert_eq!(evidence["evidence_sources"].as_array().unwrap().len(), 12);
    assert_eq!(evidence["notes"].as_array().unwrap().len(), 6);
    assert_eq!(evidence["normalization"]["candidate_limit"], 16);
    assert_eq!(evidence["normalization"]["omitted_candidates"], 4);
    assert_eq!(
        evidence["normalization"]["omitted_candidate_evidence_items"],
        10
    );
    assert_eq!(evidence["normalization"]["omitted_evidence_sources"], 4);
    assert_eq!(evidence["normalization"]["omitted_notes"], 4);
    assert!(
        evidence["normalization"]["truncated_text_fields"]
            .as_u64()
            .unwrap()
            >= 6
    );
    assert!(
        route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("Backend evidence was bounded for token safety"))
    );
}

#[test]
fn cli_agent_route_does_not_spend_backend_evidence_budget_on_duplicate_candidates() {
    let fixture = fixture_project();
    let candidates = (0..6)
        .map(|index| {
            serde_json::json!({
                "file": if index < 2 {
                    "src/main.ts".to_string()
                } else {
                    format!("graph/candidate-{index}.ts")
                },
                "evidence": (0..6)
                    .map(|item| format!("evidence-{index}-{item}"))
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidates": candidates
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let evidence = &route["routing_decision"]["backend_evidence"];
    assert_eq!(evidence["candidates"].as_array().unwrap().len(), 5);
    assert_eq!(
        evidence["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| {
                candidate["evidence"]
                    .as_array()
                    .map_or(0, |evidence| evidence.len())
            })
            .sum::<usize>(),
        24
    );
    assert_eq!(
        evidence["normalization"]["omitted_candidate_evidence_items"],
        12
    );
    assert_eq!(evidence["normalization"]["omitted_candidates"], 0);
}

#[test]
fn cli_agent_route_rejects_invalid_backend_evidence_values() {
    let fixture = fixture_project();

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            r#"{"provider":"   ","candidate_files":["src/main.ts"]}"#,
        ])
        .assert()
        .failure()
        .stderr(contains("backend evidence provider must not be empty"));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            r#"{"provider":" ","tool_results":{"search_graph":{"total":1}}}"#,
        ])
        .assert()
        .failure()
        .stderr(contains("backend evidence provider must not be empty"));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            r#"{"provider":"graph","candidate_files":["src/main.ts"],"confidence":1.5}"#,
        ])
        .assert()
        .failure()
        .stderr(contains(
            "backend evidence confidence must be between 0.0 and 1.0",
        ));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            r#"{"provider":"graph","candidate_files":["../outside.ts"]}"#,
        ])
        .assert()
        .failure()
        .stderr(contains("invalid backend evidence candidate file"));

    let long_provider = "p".repeat(129);
    let long_provider_evidence = serde_json::json!({
        "provider": long_provider,
        "candidate_files": ["src/main.ts"]
    })
    .to_string();
    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            &long_provider_evidence,
        ])
        .assert()
        .failure()
        .stderr(contains(
            "backend evidence provider must not exceed 128 characters",
        ));

    let long_file = format!("graph/{}.ts", "f".repeat(512));
    let long_file_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidate_files": [long_file]
    })
    .to_string();
    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            &long_file_evidence,
        ])
        .assert()
        .failure()
        .stderr(contains(
            "backend evidence candidate file must not exceed 512 characters",
        ));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence-json",
            r#"{"provider":"graph","tool_results":{"search_graph":{"total":1}}}"#,
        ])
        .assert()
        .failure()
        .stderr(contains(
            "backend evidence search_graph tool result must contain an array field named results",
        ));
}

#[test]
fn cli_agent_route_rejects_backend_evidence_file_and_inline_json_together() {
    let fixture = fixture_project();
    let evidence_path = fixture.path().join("backend-evidence.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/main.ts"]
        })
        .to_string(),
    )
    .unwrap();

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-evidence",
            evidence_path.to_str().unwrap(),
            "--backend-evidence-json",
            r#"{"provider":"codebase-memory-mcp","candidate_files":["src/main.ts"]}"#,
        ])
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}

#[test]
fn cli_agent_route_requires_evidence_for_backend_fallback() {
    let fixture = fixture_project();

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--backend-fallback",
        ])
        .assert()
        .failure()
        .stderr(contains("--backend-fallback requires backend evidence"));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "agent-route",
            fixture.path().to_str().unwrap(),
            "--task",
            "understand app entrypoint flow",
            "--prefer-backend-context",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "--prefer-backend-context requires backend evidence",
        ));
}

#[test]
fn cli_agent_route_uses_backend_candidate_as_explicit_fallback() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidate_files": ["missing.ts", "src/main.ts"],
        "evidence_sources": ["search_graph"],
        "evidence_count": 4
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
        "--backend-fallback",
    ]);

    assert_eq!(route["impact_status"], "complete");
    assert_eq!(
        route["routing_decision"]["seed_strategy"],
        "backend_fallback"
    );
    assert_eq!(
        route["routing_decision"]["first_seed_source"],
        "backend_fallback"
    );
    assert_eq!(route["routing_decision"]["first_file"], "src/main.ts");
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_fallback"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["selected_context_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_backend_seeded_context"
    );
    assert_eq!(route["execution_plan"][0]["status"], "ready");
    assert_eq!(
        route["impact_seed_files"],
        serde_json::json!(["src/main.ts"])
    );
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["use_as_fallback"],
        true
    );
}

#[test]
fn cli_agent_route_prefers_backend_candidate_for_bounded_context() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidates": [{
            "file": "src/ui.ts",
            "symbol": "render",
            "source": "search_graph",
            "score": 0.97,
            "reason": "graph-ranked implementation",
            "evidence": ["definition", "callers"]
        }],
        "evidence_sources": ["search_graph"],
        "evidence_count": 5
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
        "--prefer-backend-context",
    ]);

    assert_eq!(
        route["routing_decision"]["seed_strategy"],
        "backend_preferred"
    );
    assert_eq!(
        route["routing_decision"]["first_seed_source"],
        "backend_preferred"
    );
    assert_eq!(route["routing_decision"]["first_file"], "src/ui.ts");
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_preferred"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["local_first_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/ui.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["selected_context_file"],
        "src/ui.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["selected_context_files"],
        serde_json::json!(["src/ui.ts"])
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_backend_seeded_context"
    );
    assert_eq!(route["impact_seed_files"], serde_json::json!(["src/ui.ts"]));
    assert_eq!(route["impact_seed_symbols"], serde_json::json!(["render"]));
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["prefer_for_context"],
        true
    );
    assert!(
        route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("Backend preference replaced the local first-read candidate"))
    );
}

#[test]
fn cli_agent_route_routes_ranked_backend_candidates_within_budget() {
    let fixture = fixture_project();
    let mut candidates = vec![
        serde_json::json!({
            "file": "src/ui.ts",
            "symbol": "render",
            "source": "search_graph",
            "score": 0.97
        }),
        serde_json::json!({
            "file": "src/main.ts",
            "symbol": "main",
            "source": "trace_path",
            "score": 0.91
        }),
    ];
    for rank in 3..=16 {
        let file = format!("src/backend-candidate-{rank}.ts");
        let symbol = format!("backendCandidate{rank}");
        let mut source = format!("export function {symbol}() {{\n");
        for line in 1..=80 {
            source.push_str(&format!(
                "  const value_{line} = \"backend candidate {rank} context line {line}\";\n"
            ));
        }
        source.push_str("  return value_1;\n}\n");
        write_file(&fixture, &file, &source);
        candidates.push(serde_json::json!({
            "file": file,
            "symbol": symbol,
            "source": "search_graph",
            "score": 0.90 - (rank as f64 / 100.0)
        }));
    }
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "candidates": candidates
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "500",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
        "--prefer-backend-context",
    ]);

    let reading_files = route["context_pack"]["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["file"].as_str().unwrap())
        .collect::<Vec<_>>();
    let ui_position = reading_files
        .iter()
        .position(|file| *file == "src/ui.ts")
        .unwrap();
    let main_position = reading_files
        .iter()
        .position(|file| *file == "src/main.ts")
        .unwrap();

    assert!(ui_position < main_position);
    assert_eq!(route["routing_decision"]["first_file"], "src/ui.ts");
    let dispositions =
        route["routing_decision"]["backend_route_agreement"]["candidate_dispositions"]
            .as_array()
            .unwrap();
    assert_eq!(dispositions.len(), 16);
    assert_eq!(dispositions[0]["file"], "src/ui.ts");
    assert_eq!(dispositions[0]["context_status"], "selected");
    assert_eq!(dispositions[0]["next_action"], "read_selected_context");
    assert_eq!(dispositions[0]["symbol_status"], "valid");
    assert_eq!(dispositions[1]["file"], "src/main.ts");
    assert_eq!(dispositions[1]["context_status"], "selected");
    assert_eq!(dispositions[1]["next_action"], "read_selected_context");
    assert_eq!(dispositions[1]["symbol_status"], "valid");
    assert!(dispositions.iter().any(|disposition| {
        disposition["context_status"] == "omitted"
            && disposition["context_reason"] == "token_budget_exhausted"
            && disposition["next_action"] == "run_backend_candidate_context_pack"
            && disposition["symbol_status"] == "valid"
    }));
    let first_omitted = dispositions
        .iter()
        .find(|disposition| disposition["context_reason"] == "token_budget_exhausted")
        .unwrap();
    let continuation =
        &route["routing_decision"]["backend_route_agreement"]["next_candidate_continuation"];
    assert_eq!(continuation["file"], first_omitted["file"]);
    assert_eq!(continuation["rank"], first_omitted["rank"]);
    assert_eq!(continuation["symbol"], first_omitted["symbol"]);
    assert_eq!(continuation["context_reason"], "token_budget_exhausted");
    assert_eq!(
        continuation["next_action"],
        "run_backend_candidate_context_pack"
    );
    assert_eq!(continuation["suggested_tool"]["tool"], "context_pack");
    assert_eq!(
        route["routing_decision"]["continuation_source"],
        "backend_route_agreement"
    );
    assert_eq!(
        route["routing_decision"]["continuation_status"],
        "backend_candidate_available"
    );
    assert_eq!(
        route["routing_decision"]["continuation_next_action"],
        continuation["next_action"]
    );
    assert_eq!(
        continuation["suggested_tool"]["suggested_arguments"]["root"],
        route["root"]
    );
    assert_eq!(
        continuation["suggested_tool"]["suggested_arguments"]["files"],
        serde_json::json!([first_omitted["file"].as_str().unwrap()])
    );
    assert_eq!(
        continuation["suggested_tool"]["suggested_arguments"]["symbols"],
        serde_json::json!([first_omitted["symbol"].as_str().unwrap()])
    );
    assert_eq!(
        continuation["suggested_tool"]["suggested_arguments"]["token_budget"],
        4000
    );
    let continuation_step = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "run_backend_candidate_context_pack")
        .unwrap();
    assert_eq!(
        continuation_step["status"],
        "available_after_selected_context"
    );
    assert_eq!(
        continuation_step["files"],
        continuation["suggested_tool"]["suggested_arguments"]["files"]
    );
    assert_eq!(
        continuation_step["suggested_tool"],
        continuation["suggested_tool"]
    );
    assert!(
        continuation_step["instruction"]
            .as_str()
            .unwrap()
            .contains("without broad repository reading")
    );
    let impact_seed_files = route["impact_seed_files"].as_array().unwrap();
    assert!(impact_seed_files.iter().any(|file| file == "src/ui.ts"));
    assert!(impact_seed_files.iter().any(|file| file == "src/main.ts"));
    let impact_seed_symbols = route["impact_seed_symbols"].as_array().unwrap();
    assert!(impact_seed_symbols.iter().any(|symbol| symbol == "render"));
    assert!(impact_seed_symbols.iter().any(|symbol| symbol == "main"));
    assert!(
        route["context_pack"]["reading_plan"][0]["selection_reason"]
            .as_str()
            .unwrap()
            .contains("candidate rank 1")
    );
}

#[test]
fn cli_agent_route_preserves_backend_rank_after_skipping_missing_candidate() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "prefer_for_context": true,
        "candidates": [
            { "file": "src/removed.ts", "symbol": "removed" },
            { "file": "src/ui.ts", "symbol": "render" }
        ]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    assert_eq!(route["routing_decision"]["first_file"], "src/ui.ts");
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/removed.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["selected_context_files"],
        serde_json::json!(["src/ui.ts"])
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["candidate_dispositions"],
        serde_json::json!([
            {
                "file": "src/removed.ts",
                "rank": 1,
                "symbol": "removed",
                "context_status": "omitted",
                "context_reason": "missing_file",
                "next_action": "refresh_backend_evidence",
                "symbol_status": "not_checked"
            },
            {
                "file": "src/ui.ts",
                "rank": 2,
                "symbol": "render",
                "context_status": "selected",
                "context_reason": "selected_within_token_budget",
                "next_action": "read_selected_context",
                "symbol_status": "valid"
            }
        ])
    );
    assert!(
        route["context_pack"]["reading_plan"][0]["selection_reason"]
            .as_str()
            .unwrap()
            .contains("candidate rank 2")
    );
}

#[test]
fn cli_agent_route_keeps_explicit_seed_ahead_of_backend_preference() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "prefer_for_context": true,
        "candidate_files": ["src/ui.ts"]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--file",
        "src/main.ts",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    assert_eq!(route["routing_decision"]["first_file"], "src/main.ts");
    assert_ne!(
        route["routing_decision"]["seed_strategy"],
        "backend_preferred"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "conflict"
    );
    assert!(
        route["routing_decision"]["backend_route_agreement"]
            .get("selected_context_file")
            .is_none()
    );
}

#[test]
fn cli_agent_route_uses_structured_backend_candidate_metadata_in_fallback() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "use_as_fallback": true,
        "candidates": [
            {
                "file": "missing.ts",
                "source": "search_graph",
                "score": 1.0
            },
            {
                "file": "src/main.ts",
                "symbol": "main",
                "source": "trace_path",
                "score": 0.94,
                "reason": "connects startup to server construction",
                "evidence": ["definition", "outbound_calls"]
            }
        ]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    assert_eq!(route["routing_decision"]["first_file"], "src/main.ts");
    assert_eq!(
        route["routing_decision"]["backend_selected_candidate"]["symbol"],
        "main"
    );
    assert_eq!(
        route["routing_decision"]["backend_selected_candidate"]["source"],
        "trace_path"
    );
    assert_eq!(route["impact_seed_symbols"], serde_json::json!(["main"]));
    assert!(
        route["context_pack"]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| symbol["name"] == "main")
    );
    assert!(
        route["context_pack"]["reading_plan"][0]["selection_reason"]
            .as_str()
            .unwrap()
            .contains("connects startup to server construction")
    );
    assert!(
        route["execution_plan"][0]["instruction"]
            .as_str()
            .unwrap()
            .contains("evidence definition, outbound_calls")
    );
}

#[test]
fn cli_agent_route_falls_back_to_file_when_backend_symbol_is_stale() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "use_as_fallback": true,
        "candidates": [
            {
                "file": "src/main.ts",
                "symbol": "removedGraphSymbol",
                "source": "search_graph",
                "reason": "graph snapshot may be stale"
            },
            {
                "file": "src/ui.ts",
                "symbol": "removedUiSymbol",
                "source": "search_graph",
                "reason": "graph snapshot may be stale"
            }
        ]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    assert_eq!(route["routing_decision"]["first_file"], "src/main.ts");
    assert_eq!(route["impact_status"], "complete");
    assert_eq!(route["impact_seed_symbols"], serde_json::json!([]));
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_fallback"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["candidate_dispositions"],
        serde_json::json!([
            {
                "file": "src/main.ts",
                "rank": 1,
                "symbol": "removedGraphSymbol",
                "context_status": "selected",
                "context_reason": "selected_within_token_budget",
                "next_action": "read_selected_context",
                "symbol_status": "stale"
            },
            {
                "file": "src/ui.ts",
                "rank": 2,
                "symbol": "removedUiSymbol",
                "context_status": "omitted",
                "context_reason": "fallback_not_selected",
                "next_action": "use_if_fallback_context_insufficient",
                "symbol_status": "stale"
            }
        ])
    );
    let continuation =
        &route["routing_decision"]["backend_route_agreement"]["next_candidate_continuation"];
    assert_eq!(continuation["file"], "src/ui.ts");
    assert!(continuation.get("symbol").is_none());
    assert_eq!(
        continuation["suggested_tool"]["suggested_arguments"]["files"],
        serde_json::json!(["src/ui.ts"])
    );
    assert!(
        continuation["suggested_tool"]["suggested_arguments"]
            .get("symbols")
            .is_none()
    );
    let continuation_step = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "use_if_fallback_context_insufficient")
        .unwrap();
    assert_eq!(continuation_step["files"], serde_json::json!(["src/ui.ts"]));
    assert_eq!(
        continuation_step["suggested_tool"],
        continuation["suggested_tool"]
    );
    assert_eq!(
        route["routing_decision"]["continuation_source"],
        "backend_route_agreement"
    );
    assert_eq!(
        route["routing_decision"]["continuation_status"],
        "backend_candidate_available"
    );
    assert_eq!(
        route["routing_decision"]["continuation_next_action"],
        continuation_step["action"]
    );
}

#[test]
fn cli_agent_route_marks_backend_overlap_as_rank_review() {
    let fixture = fixture_project();
    let evidence_path = fixture.path().join("backend-overlap-evidence.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/server.ts", "src/main.ts"],
            "evidence_sources": ["search_graph", "trace_path"],
            "evidence_count": 6,
            "confidence": 0.82,
            "notes": ["external graph backend included local route after a different rank-1 file"]
        })
        .to_string(),
    )
    .unwrap();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence",
        evidence_path.to_str().unwrap(),
    ]);

    assert_eq!(
        route["routing_decision"]["first_file"], "src/main.ts",
        "fixture should keep the local first-read route stable"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "overlap"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/server.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["common_files"][0],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["recommended_action"],
        "read_selected_context_then_compare_backend_rank"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_selected_context_then_compare_backend_rank"
    );
    assert!(
        route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains(
                "Backend codebase-memory-mcp ranked src/server.ts before local route src/main.ts"
            ))
    );
    assert!(
        route["routing_decision"]["route_quality"]["decision_summary"]
            .as_str()
            .unwrap()
            .contains("Then read_selected_context_then_compare_backend_rank.")
    );
    let comparison_step = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "read_selected_context_then_compare_backend_rank")
        .unwrap();
    assert_eq!(
        comparison_step["status"],
        "available_after_selected_context"
    );
    assert_eq!(
        comparison_step["files"],
        serde_json::json!(["src/main.ts", "src/server.ts"])
    );
}

#[test]
fn cli_agent_route_flags_backend_evidence_conflict_before_edits() {
    let fixture = fixture_project();
    let evidence_path = fixture.path().join("backend-conflict-evidence.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/server.ts"],
            "evidence_sources": ["search_graph"],
            "evidence_count": 4,
            "confidence": 0.88,
            "notes": ["external graph backend preferred a different first file"]
        })
        .to_string(),
    )
    .unwrap();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence",
        evidence_path.to_str().unwrap(),
    ]);

    assert_eq!(
        route["routing_decision"]["first_file"], "src/main.ts",
        "fixture should keep the local first-read route stable"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "compare_backend_route_before_edits"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "conflict"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["local_first_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/server.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["recommended_action"],
        "compare_backend_route_before_edits"
    );
    assert!(
        route["routing_decision"]["backend_route_agreement"]["common_files"].is_null(),
        "empty common_files should be omitted from JSON"
    );
    assert!(
        route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("Backend codebase-memory-mcp preferred src/server.ts"))
    );
    assert!(
        route["routing_decision"]["route_quality"]["verification_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str().unwrap().contains(
                "Compare local route with backend codebase-memory-mcp candidate src/server.ts"
            ))
    );
    assert!(
        route["routing_decision"]["route_quality"]["decision_summary"]
            .as_str()
            .unwrap()
            .contains("Then compare_backend_route_before_edits.")
    );
    let comparison_step = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "compare_backend_route_before_edits")
        .unwrap();
    assert_eq!(comparison_step["status"], "required_before_edits");
    assert_eq!(
        comparison_step["files"],
        serde_json::json!(["src/main.ts", "src/server.ts"])
    );
    assert!(
        comparison_step["instruction"]
            .as_str()
            .unwrap()
            .contains("resolve the conflict before editing")
    );
}

#[test]
fn cli_agent_route_reports_backend_only_when_local_route_is_blocked() {
    let fixture = fixture_project();
    let evidence_path = fixture.path().join("backend-only-evidence.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/main.ts"],
            "evidence_sources": ["search_graph"],
            "evidence_count": 3,
            "notes": ["external graph backend still had an entry candidate"]
        })
        .to_string(),
    )
    .unwrap();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1000",
        "--force-index",
        "--backend-evidence",
        evidence_path.to_str().unwrap(),
    ]);

    assert_eq!(route["impact_status"], "skipped_invalid_seed");
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_only"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["local_first_file"],
        serde_json::Value::Null
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["backend_first_file"],
        "src/main.ts"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["recommended_action"],
        "provide_seed_or_use_backend_candidate"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "provide_seed_or_use_backend_candidate"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["evidence_count"],
        3
    );
    assert!(
        route["routing_decision"]["backend_route_agreement"]["message"]
            .as_str()
            .unwrap()
            .contains("Local routing produced no first-read file")
    );
    let backend_seed_step = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "provide_seed_or_use_backend_candidate")
        .unwrap();
    assert_eq!(backend_seed_step["status"], "required_before_edits");
    assert_eq!(
        backend_seed_step["files"],
        serde_json::json!(["src/main.ts"])
    );
    assert!(
        backend_seed_step["instruction"]
            .as_str()
            .unwrap()
            .contains("rerun agent_route before editing")
    );
}

#[test]
fn cli_agent_route_preserves_requested_minimum_token_budget() {
    let fixture = fixture_project();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "20",
        "--force-index",
    ]);

    assert_eq!(route["token_budget"].as_u64(), Some(20));
    assert_eq!(
        route["context_pack"]["budget"]["requested_token_budget"].as_u64(),
        Some(20)
    );
    assert_eq!(
        route["context_pack"]["budget"]["applied_token_budget"].as_u64(),
        Some(500)
    );
    assert_eq!(
        route["context_pack"]["budget"]["truncation_reason"],
        "minimum_budget_applied"
    );
    assert!(
        !route["context_pack"]["continuation_summary"]["status"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let continuation_message = route["context_pack"]["continuation_summary"]["message"]
        .as_str()
        .unwrap();
    assert!(continuation_message.contains("Requested token budget 20 was below the minimum"));
    assert!(continuation_message.contains("500 tokens were applied"));
    assert_agent_route_execution_plan_matches_context(&route);
}

#[test]
fn cli_agent_route_exposes_focused_impact_test_checks() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "leaf";
}

export function core() {
  return leaf();
}
"#,
    );
    write_file(
        &fixture,
        "src/core.test.ts",
        r#"
import { core } from "./core";

export function coreSpec() {
  return core();
}
"#,
    );
    write_file(&fixture, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand core behavior",
        "--token-budget",
        "1200",
        "--force-index",
        "--impact-limit",
        "20",
        "--impact-depth",
        "2",
    ]);

    let impact_checks = route["impact_analysis"]["suggested_checks"]
        .as_array()
        .unwrap();
    assert_eq!(impact_checks[0]["command"], "pnpm test -- src/core.test.ts");
    assert!(impact_checks.iter().any(|check| {
        check["kind"] == "command" && check["command"] == "pnpm test -- src/core.test.ts"
    }));
    assert_eq!(
        route["execution_plan"][3]["suggested_checks"],
        route["impact_analysis"]["suggested_checks"]
    );
    assert!(
        route["execution_plan"][3]["suggested_checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| {
                check["kind"] == "command" && check["command"] == "pnpm test -- src/core.test.ts"
            })
    );
    assert_eq!(
        route["execution_plan"][3]["suggested_tool"]["tool"],
        "impact_analysis"
    );
}

#[test]
fn cli_impact_analysis_file_seed_scans_beyond_output_limit_for_call_terms() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();

    let mut core = String::new();
    for index in 0..40 {
        core.push_str(&format!(
            "export function filler{index}() {{ return {index}; }}\n"
        ));
    }
    core.push_str(
        r#"
export function importantBehavior() {
  return "important";
}
"#,
    );
    write_file(&fixture, "src/core.ts", &core);
    write_file(
        &fixture,
        "src/consumer.ts",
        r#"
import { importantBehavior } from "./core";

export function runConsumer() {
  return importantBehavior();
}
"#,
    );

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--file",
        "src/core.ts",
        "--limit",
        "5",
        "--depth",
        "1",
        "--format",
        "summary",
    ]);

    assert!(
        impact["callers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|call| call["file"] == "src/consumer.ts" && call["callee"] == "importantBehavior"),
        "file-seed impact should scan enough symbols to find callers beyond the output limit"
    );
    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/consumer.ts"),
        "caller files should be included in the impacted file list"
    );
    assert!(
        impact["summary"]
            .as_str()
            .unwrap()
            .contains("including 1 call-related files"),
        "summary should report call-related impact for file seeds"
    );
}

#[test]
fn cli_agent_route_returns_blocked_plan_for_empty_repository() {
    let fixture = TempDir::new().unwrap();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand this repository",
        "--token-budget",
        "1000",
        "--force-index",
    ]);

    assert_eq!(route["impact_status"], "skipped_no_seed");
    assert_eq!(route["route"][2]["status"], "blocked_no_seed");
    assert_eq!(route["route"][3]["status"], "skipped_no_seed");
    assert_eq!(route["context_pack"]["seed_strategy"], "auto_no_seed");
    assert_eq!(route["context_pack"]["files"].as_array().unwrap().len(), 0);
    assert_eq!(
        route["context_pack"]["reading_plan"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["status"],
        "blocked_no_seed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_seed_file_or_symbol"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["level"],
        "blocked"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["score"].as_u64(),
        Some(0)
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "provide_seed_file_or_symbol"
    );
    assert!(
        route["routing_decision"]["route_quality"]["decision_summary"]
            .as_str()
            .unwrap()
            .contains("No first-read route was produced")
    );
    assert!(
        route["routing_decision"]["route_quality"]["verification_steps"][0]
            .as_str()
            .unwrap()
            .contains("provide a concrete seed")
    );
    assert!(
        route["routing_decision"]["route_quality"]["warnings"][0]
            .as_str()
            .unwrap()
            .contains("No reading plan")
    );
    assert!(route["current_reading_step"].is_null());
    let actions = route["execution_plan"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actions,
        vec![
            "read_selected_context",
            "use_current_reading_step_suggested_tool",
            "use_continuation_if_needed",
            "review_impact_before_edits"
        ]
    );
    assert_eq!(
        route["execution_plan"][0]["status"],
        "blocked_no_reading_plan"
    );
    assert_eq!(
        route["execution_plan"][1]["status"],
        "blocked_no_current_reading_step"
    );
    assert_eq!(
        route["execution_plan"][2]["status"],
        "manual_after_selected_context"
    );
    assert_eq!(route["execution_plan"][3]["status"], "skipped_no_seed");
    assert!(
        route["execution_plan"][0]["instruction"]
            .as_str()
            .unwrap()
            .contains("provide seed files")
    );
    assert!(
        route["execution_plan"][1]["instruction"]
            .as_str()
            .unwrap()
            .contains("provide a seed file or symbol")
    );
}

#[test]
fn cli_agent_route_returns_blocked_plan_for_invalid_seed_file() {
    let fixture = fixture_project();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1000",
        "--force-index",
    ]);

    assert_eq!(route["impact_status"], "skipped_invalid_seed");
    assert_eq!(route["route"][2]["status"], "blocked_invalid_seed");
    assert_eq!(route["route"][3]["status"], "skipped_invalid_seed");
    assert!(
        route["route"][2]["reason"]
            .as_str()
            .unwrap()
            .contains("provide_existing_seed_file_or_symbol")
    );
    assert_eq!(
        route["context_pack"]["seed_strategy"],
        "explicit_invalid_seed"
    );
    assert_eq!(
        route["context_pack"]["budget"]["truncation_reason"],
        "invalid_seed_file"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["status"],
        "blocked_invalid_seed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_existing_seed_file_or_symbol"
    );
    assert!(
        route["context_pack"]["continuation_summary"]["message"]
            .as_str()
            .unwrap()
            .contains("does/not/exist.ts")
    );
    assert_eq!(
        route["context_pack"]["selected_seeds"][0]["value"],
        "does/not/exist.ts"
    );
    assert_eq!(route["context_pack"]["files"].as_array().unwrap().len(), 0);
    assert_eq!(
        route["context_pack"]["reading_plan"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert!(route["current_reading_step"].is_null());
    assert_eq!(route["impact_seed_files"].as_array().unwrap().len(), 0);
    assert_eq!(route["impact_seed_symbols"].as_array().unwrap().len(), 0);
    assert!(
        route["execution_plan"][3]["instruction"]
            .as_str()
            .unwrap()
            .contains("explicit seed file could not be resolved")
    );
}

#[test]
fn cli_context_pack_returns_blocked_summary_for_unmatched_explicit_symbol() {
    let fixture = fixture_project();

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand nonexistent explicit symbol",
        "--symbol",
        "ThisSymbolDoesNotExist",
        "--token-budget",
        "1000",
    ]);

    assert_eq!(context["seed_strategy"], "explicit");
    assert_eq!(
        context["budget"]["truncation_reason"],
        "no_context_for_explicit_seed"
    );
    assert_eq!(
        context["continuation_summary"]["status"],
        "blocked_no_context"
    );
    assert_eq!(
        context["continuation_summary"]["next_action"],
        "provide_matching_seed_file_or_symbol"
    );
    assert!(
        context["continuation_summary"]["message"]
            .as_str()
            .unwrap()
            .contains("ThisSymbolDoesNotExist")
    );
    assert_eq!(context["files"].as_array().unwrap().len(), 0);
    assert_eq!(context["reading_plan"].as_array().unwrap().len(), 0);
}

#[test]
fn cli_agent_route_returns_blocked_plan_for_unmatched_explicit_symbol() {
    let fixture = fixture_project();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand nonexistent explicit symbol",
        "--symbol",
        "ThisSymbolDoesNotExist",
        "--token-budget",
        "1000",
        "--force-index",
    ]);

    assert_eq!(route["impact_status"], "skipped_no_context");
    assert_eq!(route["route"][2]["status"], "blocked_no_context");
    assert_eq!(route["route"][3]["status"], "skipped_no_context");
    assert!(
        route["route"][2]["reason"]
            .as_str()
            .unwrap()
            .contains("provide_matching_seed_file_or_symbol")
    );
    assert_eq!(
        route["context_pack"]["budget"]["truncation_reason"],
        "no_context_for_explicit_seed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["status"],
        "blocked_no_context"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_matching_seed_file_or_symbol"
    );
    assert_eq!(
        route["context_pack"]["selected_seeds"][0]["value"],
        "ThisSymbolDoesNotExist"
    );
    assert_eq!(route["context_pack"]["files"].as_array().unwrap().len(), 0);
    assert_eq!(
        route["context_pack"]["reading_plan"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert!(route["current_reading_step"].is_null());
    assert_eq!(route["impact_seed_files"].as_array().unwrap().len(), 0);
    assert_eq!(route["impact_seed_symbols"].as_array().unwrap().len(), 0);
    assert_eq!(
        route["execution_plan"][0]["status"],
        "blocked_no_reading_plan"
    );
    assert_eq!(route["execution_plan"][3]["status"], "skipped_no_context");
    assert!(
        route["execution_plan"][3]["instruction"]
            .as_str()
            .unwrap()
            .contains("explicit seed did not match any readable context")
    );
}

#[test]
fn cli_agent_route_points_blocked_context_at_configured_index_scope() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["src/auth.ts"]
"#,
    );
    write_file(
        &fixture,
        "src/main.ts",
        r#"
export function main() {
  return "skip";
}
"#,
    );
    write_file(
        &fixture,
        "src/auth.ts",
        r#"
export function authOnly() {
  return "keep";
}
"#,
    );

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand scoped missing main",
        "--symbol",
        "main",
        "--token-budget",
        "1000",
        "--force-index",
    ]);

    assert_eq!(route["index_report"]["indexed_files"].as_u64(), Some(1));
    assert_eq!(
        route["index_report"]["index_scope"]["enabled"].as_bool(),
        Some(true)
    );
    assert_eq!(
        route["index_report"]["index_scope"]["walk_roots"][0],
        "src/auth.ts"
    );
    assert_eq!(route["route"][2]["status"], "blocked_no_context");
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "check_index_scope_or_provide_matching_seed"
    );
    let continuation_message = route["context_pack"]["continuation_summary"]["message"]
        .as_str()
        .unwrap();
    assert!(continuation_message.contains("Index scope is enabled"));
    assert!(continuation_message.contains("src/auth.ts"));
    assert!(
        route["route"][2]["reason"]
            .as_str()
            .unwrap()
            .contains("check_index_scope_or_provide_matching_seed")
    );
    assert!(
        route["execution_plan"][2]["instruction"]
            .as_str()
            .unwrap()
            .contains("Index scope is enabled")
    );
}

#[test]
fn cli_context_pack_blocks_existing_task_path_outside_index_scope() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["src/auth.ts"]
"#,
    );
    write_file(
        &fixture,
        "src/main.ts",
        r#"
export function main() {
  return "skip";
}
"#,
    );
    write_file(
        &fixture,
        "src/auth.ts",
        r#"
export function authOnly() {
  return "keep";
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"].as_u64(), Some(1));

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "inspect src/main.ts before editing startup",
        "--token-budget",
        "1000",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_path_unindexed");
    assert_eq!(context["selected_seeds"][0]["value"], "src/main.ts");
    assert_eq!(
        context["selected_seeds"][0]["source"],
        "task_path_unindexed"
    );
    assert_eq!(
        context["budget"]["truncation_reason"],
        "unindexed_task_path"
    );
    assert_eq!(
        context["continuation_summary"]["status"],
        "blocked_unindexed_task_path"
    );
    assert_eq!(
        context["continuation_summary"]["next_action"],
        "index_or_update_scope_for_task_path"
    );
    assert!(context["files"].as_array().unwrap().is_empty());
    assert!(context["reading_plan"].as_array().unwrap().is_empty());
}

#[test]
fn cli_agent_route_blocks_existing_task_path_outside_index_scope() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["src/auth.ts"]
"#,
    );
    write_file(
        &fixture,
        "src/main.ts",
        r#"
export function main() {
  return "skip";
}
"#,
    );
    write_file(
        &fixture,
        "src/auth.ts",
        r#"
export function authOnly() {
  return "keep";
}
"#,
    );

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "inspect src/main.ts before editing startup",
        "--token-budget",
        "1000",
        "--force-index",
    ]);

    assert_eq!(route["index_report"]["indexed_files"].as_u64(), Some(1));
    assert_eq!(route["route"][2]["status"], "blocked_unindexed_task_path");
    assert_eq!(route["impact_status"], "skipped_unindexed_task_path");
    assert_eq!(
        route["context_pack"]["seed_strategy"],
        "auto_task_path_unindexed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "index_or_update_scope_for_task_path"
    );
    let continuation_message = route["context_pack"]["continuation_summary"]["message"]
        .as_str()
        .unwrap();
    assert!(continuation_message.contains("src/main.ts"));
    assert!(continuation_message.contains("Index scope is enabled"));
    assert!(
        route["execution_plan"][2]["instruction"]
            .as_str()
            .unwrap()
            .contains("Index scope is enabled")
    );
    assert!(
        route["execution_plan"][3]["instruction"]
            .as_str()
            .unwrap()
            .contains("task path seed is not indexed")
    );
}

#[test]
fn cli_agent_route_keeps_entrypoint_companion_for_task_match() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    std::fs::write(
        fixture.path().join("package.json"),
        r#"{
  "type": "module",
  "scripts": {
    "start": "tsx src/main.ts"
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/main.ts"),
        r#"import { bootRouter } from "./router";

export function main() {
  return bootRouter();
}

main();
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/router.ts"),
        r#"import { authenticate } from "./auth";

export function bootRouter() {
  return authenticate("demo-user");
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/auth.ts"),
        r#"export function authenticate(user: string) {
  return { user, status: "accepted" };
}
"#,
    )
    .unwrap();
    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand router auth flow",
        "--token-budget",
        "1600",
        "--force-index",
    ]);

    assert_eq!(route["context_pack"]["seed_strategy"], "auto_task_match");
    assert_eq!(
        route["context_pack"]["selected_seeds"][0]["value"],
        "src/router.ts"
    );
    assert_eq!(
        route["context_pack"]["selected_seeds"][0]["source"],
        "task_match"
    );
    assert_eq!(
        route["context_pack"]["selected_seeds"][1]["value"],
        "src/main.ts"
    );
    assert_eq!(
        route["context_pack"]["selected_seeds"][1]["source"],
        "overview_entrypoint"
    );
    assert_eq!(route["context_pack"]["files"][0]["file"], "src/router.ts");
    assert!(
        route["context_pack"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/main.ts")
    );
    assert_eq!(
        route["context_pack"]["reading_plan"][0]["file"],
        "src/router.ts"
    );
    assert!(
        route["context_pack"]["reading_plan"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["file"] == "src/main.ts")
    );
}

#[test]
fn cli_context_pack_routes_to_late_matching_symbol_over_config_shell() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    write_file(
        &fixture,
        "src/main.ts",
        r#"
import { loadImpactConfig } from "./config";

export function main() {
  return loadImpactConfig();
}

main();
"#,
    );
    write_file(
        &fixture,
        "src/index.ts",
        r#"
export const firstReadRoutingQuality = {
  first: "read",
  routing: "quality"
};
"#,
    );
    write_file(
        &fixture,
        "src/config.ts",
        r#"
export function loadImpactConfig() {
  return {
    impact_analysis: true,
    suggested_checks: ["pnpm test"],
    routing_quality: "configuration shell"
  };
}
"#,
    );
    let mut tools_source = String::new();
    for index in 0..40 {
        tools_source.push_str(&format!(
            "export function helper{index}() {{ return {index}; }}\n"
        ));
    }
    tools_source.push_str(
        r#"
export function impactSuggestedChecksRouter() {
  return {
    impact: "analysis",
    suggested: "checks",
    routing: "implementation quality"
  };
}
"#,
    );
    std::fs::write(fixture.path().join("src/tools.ts"), tools_source).unwrap();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "improve impact analysis suggested checks routing quality",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(
        context["selected_seeds"][0]["value"], "src/tools.ts",
        "late matching implementation symbols should beat config-shell text matches"
    );
    assert!(
        context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "impact")
    );
    assert!(
        context["selected_seeds"][0]["matched_symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| symbol == "impactSuggestedChecksRouter")
    );
    assert_eq!(context["selected_seeds"][1]["value"], "src/main.ts");
    assert_eq!(context["files"][0]["file"], "src/tools.ts");
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("first-read handoff")
    );
    assert!(
        context["reading_plan"][0]["question"]
            .as_str()
            .unwrap()
            .contains("agent first-read workflow")
    );

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "improve impact analysis suggested checks routing quality",
        "--token-budget",
        "1600",
        "--impact-limit",
        "20",
        "--impact-depth",
        "2",
    ]);
    assert!(
        route["impact_seed_symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| symbol == "impactSuggestedChecksRouter")
    );
    assert!(
        route["impact_analysis"]["seed_symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| symbol == "impactSuggestedChecksRouter")
    );
    assert!(
        route["current_reading_step"]["question"]
            .as_str()
            .unwrap()
            .contains("agent first-read workflow")
    );
}

#[test]
fn cli_context_pack_routes_agent_context_tasks_to_core_source_over_evidence_scripts() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/main.ts",
        r#"
import { createContextRouter } from "./tools";

export function main() {
  return createContextRouter("agent context routing");
}

main();
"#,
    );
    let mut tools_source = String::new();
    for index in 0..40 {
        tools_source.push_str(&format!(
            "export function helper{index}() {{ return {index}; }}\n"
        ));
    }
    tools_source.push_str(
        r#"
export function createContextRouter(task: string) {
  return routeAgentContextPack(task);
}

export function routeAgentContextPack(task: string) {
  return {
    task,
    contextPack: "bounded",
    readingPlan: "handoff",
    routeQuality: "implementation"
  };
}
"#,
    );
    write_file(&fixture, "src/tools.ts", &tools_source);
    write_file(
        &fixture,
        "scripts/agent-context-routing-evidence.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

agent_route_metric() {
  echo "agent context route router routing metric"
}

validate_context_pack_quality_artifact() {
  echo "agent context pack quality evidence"
}

main() {
  agent_route_metric
  validate_context_pack_quality_artifact
}

main "$@"
"#,
    );
    write_file(
        &fixture,
        "scripts/demo-output-live-sync-smoke.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

sync_live_agent_route_demo_output() {
  echo "agent_route context_pack routing_decision first-read read-less evidence"
  echo "context routing demo output should stay synchronized"
}

validate_demo_context_router_contract() {
  echo "selected context route quality reading plan impact analysis"
}

main() {
  sync_live_agent_route_demo_output
  validate_demo_context_router_contract
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand agent context routing",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(
        context["selected_seeds"][0]["value"], "src/tools.ts",
        "agent context routing should start from the implementation, not evidence scripts"
    );
    assert_eq!(context["files"][0]["file"], "src/tools.ts");
    assert!(
        !context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "scripts/agent-context-routing-evidence.sh"),
        "non-evidence agent context routing should not spend context budget on evidence scripts"
    );
    assert!(
        context["files"][0]["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| range["reason"]
                .as_str()
                .unwrap()
                .contains("routeAgentContextPack")),
        "seed-file ranges should include the late task-matching routing symbol"
    );
}

#[test]
fn cli_context_pack_routes_indexing_tasks_to_index_source_over_demo_scripts() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/index.ts",
        r#"
export function buildProjectIndex(root: string) {
  return parseSourceFiles(root);
}

export function parseSourceFiles(root: string) {
  return { root, indexedFiles: 42 };
}
"#,
    );
    write_file(
        &fixture,
        "scripts/agent-router-demo.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "indexing pipeline demo output"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand indexing pipeline",
        "--token-budget",
        "1200",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/index.ts");
    assert_eq!(context["files"][0]["file"], "src/index.ts");
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("project indexing"),
        "indexing tasks should get indexing-specific reading focus"
    );
    assert!(
        context["reading_plan"][0]["question"]
            .as_str()
            .unwrap()
            .contains("files scanned"),
        "indexing tasks should get indexing-specific reading guidance"
    );
}

#[test]
fn cli_context_pack_routes_storage_migrations_to_storage_source_over_evidence_scripts() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/storage.ts",
        r#"
export function migrateDatabaseSchema() {
  return applyStorageMigration();
}

export function applyStorageMigration() {
  return "schema migrated";
}
"#,
    );
    write_file(
        &fixture,
        "scripts/adoption-evidence.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "storage migrations adoption evidence"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand storage migrations",
        "--token-budget",
        "1200",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/storage.ts");
    assert_eq!(context["files"][0]["file"], "src/storage.ts");
    assert!(
        !context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "scripts/adoption-evidence.sh"),
        "storage migration tasks should not spend context budget on adoption evidence scripts"
    );
}

#[test]
fn cli_context_pack_does_not_route_semantic_indexing_as_plain_index_pipeline() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/index.ts",
        r#"
export function buildProjectIndex(root: string) {
  return { root };
}
"#,
    );
    write_file(
        &fixture,
        "src/embedding.ts",
        r#"
export function generateSemanticEmbeddings(chunks: string[]) {
  return chunks.map((chunk) => ({ chunk, vector: [1, 0] }));
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand semantic indexing",
        "--token-budget",
        "1200",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/embedding.ts");
    assert_eq!(context["files"][0]["file"], "src/embedding.ts");
}

#[test]
fn cli_context_pack_keeps_semantic_search_context_in_source_files() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/tools.ts",
        r#"
import { generateSemanticEmbeddings } from "./embedding";
import { loadSemanticChunks } from "./storage";

export function semanticSearchFallback(query: string) {
  return generateSemanticEmbeddings(loadSemanticChunks(query));
}
"#,
    );
    write_file(
        &fixture,
        "src/embedding.ts",
        r#"
export function generateSemanticEmbeddings(chunks: string[]) {
  return chunks.map((chunk) => ({ chunk, vector: [1, 0] }));
}
"#,
    );
    write_file(
        &fixture,
        "src/storage.ts",
        r#"
export function loadSemanticChunks(query: string) {
  return [query];
}
"#,
    );
    write_file(
        &fixture,
        "scripts/adoption-evidence.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "semantic search fallback adoption evidence"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand semantic search fallback",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/tools.ts");
    assert_eq!(context["files"][0]["file"], "src/tools.ts");
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("semantic search orchestration"),
        "semantic search tasks should get semantic-specific reading focus"
    );
    assert!(
        context["reading_plan"][0]["question"]
            .as_str()
            .unwrap()
            .contains("semantic searches routed"),
        "semantic search tasks should get semantic-specific reading guidance"
    );
    assert!(
        !context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "scripts/adoption-evidence.sh"),
        "semantic search tasks should keep context budget on implementation sources"
    );
}

#[test]
fn cli_context_pack_routes_dependency_graph_generation_to_index_source_over_mcp_wrapper() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/index.ts",
        r#"
export function extractDependencies(source: string) {
  return source.includes("import") ? ["dependency edge"] : [];
}

export function generateDependencyGraph(source: string) {
  return extractDependencies(source);
}
"#,
    );
    write_file(
        &fixture,
        "src/storage.ts",
        r#"
export function dependencyGraph(edges: string[]) {
  return { edges };
}
"#,
    );
    write_file(
        &fixture,
        "src/mcp.ts",
        r#"
export function dependency_graph(args: Record<string, string>) {
  return args;
}
"#,
    );
    write_file(
        &fixture,
        "scripts/adoption-evidence.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "dependency graph generation adoption evidence"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand dependency graph generation",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/index.ts");
    assert_eq!(context["files"][0]["file"], "src/index.ts");
    assert!(
        context["reading_plan"][0]["focus"]
            .as_str()
            .unwrap()
            .contains("dependency graph extraction"),
        "dependency graph tasks should get graph-specific reading focus"
    );
    assert!(
        context["reading_plan"][0]["question"]
            .as_str()
            .unwrap()
            .contains("dependency edges extracted"),
        "dependency graph tasks should get graph-specific reading guidance"
    );
    assert!(
        !context["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "scripts/adoption-evidence.sh"),
        "dependency graph generation should not spend context budget on adoption evidence scripts"
    );
}

#[test]
fn cli_context_pack_routes_code_analysis_capability_tasks_to_core_sources() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/storage.ts",
        r#"
export function entrypointCandidates(files: string[]) {
  return files.filter((file) => file.endsWith("main.ts"));
}

export function projectOverview(files: string[]) {
  return { entrypoints: entrypointCandidates(files) };
}

export function searchSymbols(query: string) {
  return [{ name: query, kind: "function" }];
}
"#,
    );
    write_file(
        &fixture,
        "src/tools.ts",
        r#"
import { searchSymbols } from "./storage";

export function symbolSearchValue(query: string) {
  return searchSymbols(query);
}

export function findReferencesValue(symbol: string) {
  return [{ symbol, kind: "call" }];
}

export function callersValue(symbol: string) {
  return [{ caller: "route", callee: symbol }];
}

export function calleesValue(symbol: string) {
  return [{ caller: symbol, callee: "leaf" }];
}
"#,
    );
    write_file(
        &fixture,
        "src/index.ts",
        r#"
export function parseFile(path: string, language: string) {
  return { path, language, ast: "tree-sitter" };
}

export function resolvePackageImport(source: string) {
  return source.includes("import") ? "resolved-package" : "local-file";
}
"#,
    );
    write_file(
        &fixture,
        "src/language.ts",
        r#"
export function detectLanguage(path: string) {
  return path.endsWith(".ts") ? "typescript" : "unknown";
}
"#,
    );
    write_file(
        &fixture,
        "src/embedding.ts",
        r#"
export function embeddingProviderStatus(provider: string) {
  return { provider, status: "ready", diagnostics: [] };
}
"#,
    );
    write_file(
        &fixture,
        "src/mcp.ts",
        r#"
export function optionalStringArray(argumentsJson: unknown, key: string) {
  // JSON binding validation checks MCP tool arguments before dispatch.
  return { argumentsJson, key, schema: "array-of-strings" };
}
"#,
    );
    write_file(
        &fixture,
        "scripts/framework-entrypoint-demo.sh",
        r#"
#!/usr/bin/env bash
echo "project overview entrypoint detection demo script"
echo "symbol search implementation and file parsing language support"
"#,
    );
    write_file(
        &fixture,
        "scripts/agent-router-demo.sh",
        r#"
#!/usr/bin/env bash
echo "json binding validation package import resolution demo"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 8);

    for (task, expected_file) in [
        (
            "understand project overview entrypoint detection",
            "src/storage.ts",
        ),
        ("understand symbol search implementation", "src/tools.ts"),
        ("understand find references classification", "src/tools.ts"),
        (
            "understand callers callees call graph traversal",
            "src/tools.ts",
        ),
        (
            "understand embedding provider status reporting",
            "src/embedding.ts",
        ),
        ("understand file parsing language support", "src/index.ts"),
        ("understand package import resolution", "src/index.ts"),
        ("understand json binding validation", "src/mcp.ts"),
    ] {
        let context = run_json([
            "context-pack",
            fixture.path().to_str().unwrap(),
            "--task",
            task,
            "--token-budget",
            "1800",
        ]);

        assert_eq!(context["seed_strategy"], "auto_task_match", "{task}");
        assert_eq!(
            context["selected_seeds"][0]["value"], expected_file,
            "{task}"
        );
        assert_eq!(context["files"][0]["file"], expected_file, "{task}");
        assert!(
            !context["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|file| file["file"]
                    .as_str()
                    .is_some_and(|path| path.starts_with("scripts/"))),
            "{task} should not spend context budget on demo scripts"
        );
        if task == "understand file parsing language support" {
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("source files parsed"),
                "file parsing tasks should get parsing-specific reading guidance"
            );
        } else if task == "understand symbol search implementation" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("symbol lookup"),
                "symbol search tasks should get symbol-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("symbol queries matched"),
                "symbol search tasks should get symbol-specific reading guidance"
            );
        } else if task == "understand find references classification" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("reference search"),
                "reference search tasks should get reference-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("references found"),
                "reference search tasks should get reference-specific reading guidance"
            );
        } else if task == "understand callers callees call graph traversal" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("call graph extraction"),
                "call graph traversal tasks should get call-graph-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("callers or callees traversed"),
                "call graph traversal tasks should get call-graph-specific reading guidance"
            );
        } else if task == "understand embedding provider status reporting" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("embedding provider status"),
                "embedding provider status tasks should get provider-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("provider status detected"),
                "embedding provider status tasks should get provider-specific reading guidance"
            );
        } else if task == "understand package import resolution" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("import parsing"),
                "import resolution tasks should get import-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("imports parsed"),
                "import resolution tasks should get import-specific reading guidance"
            );
        } else if task == "understand project overview entrypoint detection" {
            assert!(
                context["reading_plan"][0]["focus"]
                    .as_str()
                    .unwrap()
                    .contains("entrypoint detection"),
                "project overview tasks should get overview-specific reading focus"
            );
            assert!(
                context["reading_plan"][0]["question"]
                    .as_str()
                    .unwrap()
                    .contains("entrypoint candidates"),
                "project overview tasks should get overview-specific reading guidance"
            );
        }
    }
}

#[test]
fn cli_context_pack_expands_common_agent_task_aliases() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    std::fs::write(
        fixture.path().join("src/main.ts"),
        r#"import { bootRouter } from "./router";
import { loadConfig } from "./config";

export function main() {
  return bootRouter(loadConfig());
}

main();
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/router.ts"),
        r#"import { authenticate } from "./auth";

export function bootRouter(settings: Record<string, string>) {
  return authenticate(settings.user);
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/auth.ts"),
        r#"export function authenticate(user: string) {
  return { user, status: "accepted" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/permissions.ts"),
        r#"export function authorizePermission(token: string) {
  // Authorization permission checks validate the bearer token.
  return { token, permission: "admin" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/config.ts"),
        r#"export function loadConfig() {
  return { user: "demo-user" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/feature_flags.ts"),
        r#"export function evaluateFeatureFlag(flagKey: string) {
  // Feature flag rollout toggles experiment variants for selected users.
  return { flagKey, rollout: "gradual", variant: "enabled" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/network.ts"),
        r#"export function configureProxyTransport(proxyUrl: string) {
  // Network HTTP adapter follows redirects through the configured proxy.
  return { proxy: proxyUrl, redirect: "follow", transport: "http" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/tls_transport.ts"),
        r#"export function verifyTlsCertificate(certPath: string) {
  // TLS SSL certificate verification configures the trusted CA bundle.
  return { certPath, verify: true, tls: "required" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/adapter.test.ts"),
        r#"export function adapterCoverageSpec() {
  // Test coverage asserts HTTP adapter regression behavior.
  return { adapter: "http", coverage: "regression", assertions: true };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/validation.ts"),
        r#"export function bindJsonValidationSchema(payload: unknown) {
  // JSON binding validates payloads against the request schema.
  return { payload, schema: "user", validator: "strict" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/database.ts"),
        r#"export function connectDatabase() {
  // Persist user records in durable storage.
  return { repository: "users", storage: "postgres" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/errors.ts"),
        r#"export function handleError(error: Error) {
  // Retry timeout failures before falling back to the caller.
  return { retry: true, timeout: error.message.includes("timeout") };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/retry_transport.ts"),
        r#"export function sendWithRetryTimeout(request: { url: string }) {
  // Transport send path handles retry failures and timeout recovery.
  return { request, retry: "once", timeout: 30, recovery: "fallback" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/router.test.ts"),
        r#"import { bootRouter } from "./router";

export function routerRegressionSpec() {
  // Regression coverage for router behavior.
  return bootRouter({ user: "demo-user" });
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/application.ts"),
        r#"export function attach(handler: unknown) {
  // Registers middleware before routes are mounted.
  return { handler, stage: "middleware" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/handler.ts"),
        r#"export function handleRequest(request: { path: string }) {
  // API endpoint handler returns the response payload.
  return { response: request.path };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/cache.ts"),
        r#"export function readCachedProfile(cacheKey: string) {
  // Cache performance path optimizes latency for repeated reads.
  return { cacheKey, latency: "low", optimization: "memory-cache" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/telemetry.ts"),
        r#"export function recordTelemetry(eventName: string) {
  // Observability telemetry emits logs and metrics for monitoring.
  return { eventName, logs: true, metrics: "request_count", trace: "span" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/cookies.ts"),
        r#"export function mergeCookieJar(cookieHeader: string) {
  // Cookie jar handling preserves HTTP state across requests.
  return { cookieHeader, cookiejar: "merged", cookies: true };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/structures.ts"),
        r#"export class CaseInsensitiveHeaders {
  // Headers are stored case-insensitively for HTTP lookups.
  get(headerName: string) {
    return headerName.toLowerCase();
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/security.ts"),
        r#"export function sanitizeSecurityInput(input: string) {
  // Security sanitization guards against injection vulnerabilities.
  return input.replace(/[<>]/g, "");
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/billing.ts"),
        r#"export function createCheckoutSession(subscriptionId: string) {
  // Billing payment checkout creates a subscription invoice.
  return { subscription: subscriptionId, payment: "pending", invoice: "draft" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/component.tsx"),
        r#"export function UserCardComponent() {
  // Frontend UI component renders the profile page layout.
  return <section className="profile-card">profile</section>;
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/renderer.ts"),
        r#"export function renderHttpResponse(templateName: string) {
  // Response rendering selects templates and produces output formats.
  return { templateName, output: "html", response: "rendered" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/response.ts"),
        r#"export function renderResponseBody(viewName: string) {
  // HTTP response rendering boundary sends rendered output to the client.
  return { viewName, response: "html", output: "sent" };
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/worker.ts"),
        r#"export function runBackgroundWorker(queueName: string) {
  // Background job worker drains the scheduled queue.
  return { queue: queueName, job: "scheduled-refresh" };
}
"#,
    )
    .unwrap();
    std::fs::create_dir_all(fixture.path().join("docs")).unwrap();
    std::fs::write(
        fixture.path().join("docs/usage.ts"),
        r#"export const usageGuide = {
  documentation: "setup examples and usage workflows",
};
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let routing_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand routing behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(routing_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        routing_context["selected_seeds"][0]["value"],
        "src/router.ts"
    );
    assert!(
        routing_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "router")
    );
    assert_eq!(routing_context["files"][0]["file"], "src/router.ts");

    let auth_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand authentication behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(auth_context["seed_strategy"], "auto_task_match");
    assert_eq!(auth_context["selected_seeds"][0]["value"], "src/auth.ts");
    assert!(
        auth_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "auth")
    );
    assert_eq!(auth_context["files"][0]["file"], "src/auth.ts");
    let auth_question = auth_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(auth_question.contains("authentication decisions"));
    assert!(auth_question.contains("session boundaries"));

    let authorization_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand authorization permissions",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(authorization_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        authorization_context["selected_seeds"][0]["value"],
        "src/permissions.ts"
    );
    assert!(
        authorization_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "permission")
    );
    assert_eq!(
        authorization_context["files"][0]["file"],
        "src/permissions.ts"
    );
    let authorization_question = authorization_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(authorization_question.contains("authentication decisions"));
    assert!(authorization_question.contains("session boundaries"));

    let access_control_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand access control rules",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(access_control_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        access_control_context["selected_seeds"][0]["value"],
        "src/permissions.ts"
    );
    assert!(
        access_control_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "permission")
    );
    assert_eq!(
        access_control_context["files"][0]["file"],
        "src/permissions.ts"
    );
    let access_control_question = access_control_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(access_control_question.contains("authentication decisions"));
    assert!(access_control_question.contains("session boundaries"));

    let settings_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand application settings",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(settings_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        settings_context["selected_seeds"][0]["value"],
        "src/config.ts"
    );
    assert!(
        settings_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "config")
    );
    assert_eq!(settings_context["files"][0]["file"], "src/config.ts");
    let settings_question = settings_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(settings_question.contains("configuration options"));
    assert!(settings_question.contains("environment inputs"));

    let feature_flag_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand feature flag rollout",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(feature_flag_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        feature_flag_context["selected_seeds"][0]["value"],
        "src/feature_flags.ts"
    );
    assert!(
        feature_flag_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "flag")
    );
    assert_eq!(
        feature_flag_context["files"][0]["file"],
        "src/feature_flags.ts"
    );
    let feature_flag_question = feature_flag_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(feature_flag_question.contains("feature flags"));
    assert!(feature_flag_question.contains("rollouts"));

    let network_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand proxy redirect transport",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(network_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        network_context["selected_seeds"][0]["value"],
        "src/network.ts"
    );
    assert!(
        network_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "proxy")
    );
    assert_eq!(network_context["files"][0]["file"], "src/network.ts");
    let network_question = network_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(network_question.contains("network requests"));
    assert!(network_question.contains("proxies"));
    assert!(network_question.contains("redirects"));

    let tls_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand ssl certificate verification",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(tls_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        tls_context["selected_seeds"][0]["value"],
        "src/tls_transport.ts"
    );
    assert!(
        tls_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "certificate")
    );
    assert_eq!(tls_context["files"][0]["file"], "src/tls_transport.ts");
    let tls_question = tls_context["reading_plan"][0]["question"].as_str().unwrap();
    assert!(tls_question.contains("TLS certificates"));
    assert!(tls_question.contains("verification decisions"));

    let adapter_test_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand adapter test coverage",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(adapter_test_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        adapter_test_context["selected_seeds"][0]["value"],
        "src/adapter.test.ts"
    );
    assert_eq!(
        adapter_test_context["files"][0]["file"],
        "src/adapter.test.ts"
    );
    let adapter_test_focus = adapter_test_context["reading_plan"][0]["focus"]
        .as_str()
        .unwrap();
    let adapter_test_question = adapter_test_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(adapter_test_focus.contains("regression coverage"));
    assert!(adapter_test_question.contains("assertions"));
    assert!(adapter_test_question.contains("regression cases"));

    let validation_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand json binding validation",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(validation_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        validation_context["selected_seeds"][0]["value"],
        "src/validation.ts"
    );
    assert!(
        validation_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "validation")
    );
    assert_eq!(validation_context["files"][0]["file"], "src/validation.ts");
    let validation_question = validation_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(validation_question.contains("inputs validated"));
    assert!(validation_question.contains("schemas applied"));

    let persistence_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand persistence behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(persistence_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        persistence_context["selected_seeds"][0]["value"],
        "src/database.ts"
    );
    assert!(
        persistence_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "database")
    );
    assert_eq!(persistence_context["files"][0]["file"], "src/database.ts");
    let persistence_question = persistence_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(persistence_question.contains("database access"));
    assert!(persistence_question.contains("storage boundaries"));

    let error_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "debug retry timeout handling",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(error_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        error_context["selected_seeds"][0]["value"],
        "src/retry_transport.ts"
    );
    assert!(
        error_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "retry")
    );
    assert_eq!(error_context["files"][0]["file"], "src/retry_transport.ts");
    let error_question = error_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(error_question.contains("retries"));
    assert!(error_question.contains("timeouts"));
    assert!(error_question.contains("recovery decisions"));

    let coverage_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "find regression coverage",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(coverage_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        coverage_context["selected_seeds"][0]["value"],
        "src/router.test.ts"
    );
    assert!(
        coverage_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "regression")
    );
    assert_eq!(coverage_context["files"][0]["file"], "src/router.test.ts");
    let coverage_question = coverage_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(coverage_question.contains("assertions"));
    assert!(coverage_question.contains("regression cases"));

    let handler_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand api handler behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(handler_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        handler_context["selected_seeds"][0]["value"],
        "src/handler.ts"
    );
    assert!(
        handler_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "api")
    );
    assert_eq!(handler_context["files"][0]["file"], "src/handler.ts");
    let handler_question = handler_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(handler_question.contains("API requests"));
    assert!(handler_question.contains("controller boundaries"));

    let performance_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand cache performance latency",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(performance_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        performance_context["selected_seeds"][0]["value"],
        "src/cache.ts"
    );
    assert!(
        performance_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "cache")
    );
    assert_eq!(performance_context["files"][0]["file"], "src/cache.ts");
    let performance_question = performance_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(performance_question.contains("cache reads"));
    assert!(performance_question.contains("latency"));

    let observability_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand observability telemetry logs",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(observability_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        observability_context["selected_seeds"][0]["value"],
        "src/telemetry.ts"
    );
    assert!(
        observability_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "telemetry")
    );
    assert_eq!(
        observability_context["files"][0]["file"],
        "src/telemetry.ts"
    );
    let observability_question = observability_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(observability_question.contains("logs"));
    assert!(observability_question.contains("metrics"));

    let cookie_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand cookie jar behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(cookie_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        cookie_context["selected_seeds"][0]["value"],
        "src/cookies.ts"
    );
    assert!(
        cookie_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "cookie")
    );
    assert_eq!(cookie_context["files"][0]["file"], "src/cookies.ts");
    let cookie_question = cookie_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(cookie_question.contains("cookies"));
    assert!(cookie_question.contains("HTTP state"));

    let headers_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand headers case insensitive behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(headers_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        headers_context["selected_seeds"][0]["value"],
        "src/structures.ts"
    );
    assert!(
        headers_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "headers")
    );
    assert_eq!(headers_context["files"][0]["file"], "src/structures.ts");
    let headers_question = headers_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(headers_question.contains("headers"));
    assert!(headers_question.contains("HTTP state"));

    let security_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand security sanitization vulnerabilities",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(security_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        security_context["selected_seeds"][0]["value"],
        "src/security.ts"
    );
    assert!(
        security_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "security")
    );
    assert_eq!(security_context["files"][0]["file"], "src/security.ts");
    let security_question = security_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(security_question.contains("security checks"));
    assert!(security_question.contains("vulnerability boundaries"));

    let billing_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand checkout subscription payment",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(billing_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        billing_context["selected_seeds"][0]["value"],
        "src/billing.ts"
    );
    assert!(
        billing_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "checkout")
    );
    assert_eq!(billing_context["files"][0]["file"], "src/billing.ts");
    let billing_question = billing_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(billing_question.contains("billing"));
    assert!(billing_question.contains("subscription decisions"));

    let frontend_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand frontend component rendering",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(frontend_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        frontend_context["selected_seeds"][0]["value"],
        "src/component.tsx"
    );
    assert!(
        frontend_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "component")
    );
    assert_eq!(frontend_context["files"][0]["file"], "src/component.tsx");
    let frontend_question = frontend_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(frontend_question.contains("frontend component"));
    assert!(frontend_question.contains("layout behavior"));

    let response_rendering_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand response rendering behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(
        response_rendering_context["seed_strategy"],
        "auto_task_match"
    );
    assert_eq!(
        response_rendering_context["selected_seeds"][0]["value"],
        "src/response.ts"
    );
    assert!(
        response_rendering_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "rendering")
    );
    assert_eq!(
        response_rendering_context["files"][0]["file"],
        "src/response.ts"
    );
    let response_rendering_question = response_rendering_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(response_rendering_question.contains("responses rendered"));
    assert!(response_rendering_question.contains("output formats"));

    let background_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand background job queue",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(background_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        background_context["selected_seeds"][0]["value"],
        "src/worker.ts"
    );
    assert!(
        background_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "worker")
    );
    assert_eq!(background_context["files"][0]["file"], "src/worker.ts");
    let background_question = background_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(background_question.contains("background jobs"));
    assert!(background_question.contains("scheduled runs"));

    let docs_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand documentation usage",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(docs_context["seed_strategy"], "auto_task_match");
    assert_eq!(docs_context["selected_seeds"][0]["value"], "docs/usage.ts");
    assert!(
        docs_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "docs")
    );
    assert_eq!(docs_context["files"][0]["file"], "docs/usage.ts");
    let docs_question = docs_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(docs_question.contains("usage"));
    assert!(docs_question.contains("documented workflow"));

    let middleware_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand middleware behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(middleware_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        middleware_context["selected_seeds"][0]["value"],
        "src/application.ts"
    );
    assert!(
        middleware_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "middleware")
    );
    assert_eq!(middleware_context["files"][0]["file"], "src/application.ts");
    let middleware_question = middleware_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(middleware_question.contains("middleware"));
    assert!(middleware_question.contains("handler boundaries"));

    let startup_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand startup flow",
        "--token-budget",
        "1600",
    ]);
    assert_ne!(
        startup_context["selected_seeds"][0]["value"],
        "src/application.ts"
    );
    assert!(
        startup_context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["value"] == "src/main.ts")
    );
    assert_ne!(startup_context["files"][0]["file"], "src/application.ts");
    let startup_question = startup_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(startup_question.contains("startup entrypoint"));
    assert!(startup_question.contains("initialization sequence"));
}

#[test]
fn cli_context_pack_uses_task_signal_word_boundaries() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    std::fs::write(
        fixture.path().join("src/catalog.ts"),
        r#"export function loadCatalog() {
  return { catalog: ["alpha", "beta"] };
}
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand catalog behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(context["seed_strategy"], "auto_task_match");
    assert_eq!(context["selected_seeds"][0]["value"], "src/catalog.ts");
    assert_eq!(context["files"][0]["file"], "src/catalog.ts");
    let question = context["reading_plan"][0]["question"].as_str().unwrap();
    assert!(question.contains("entrypoints"));
    assert!(!question.contains("logs"));
    assert!(!question.contains("metrics"));
}

#[test]
fn cli_context_pack_routes_external_beta_tasks_to_shell_scripts() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("Formula")).unwrap();
    std::fs::create_dir_all(fixture.path().join("scripts")).unwrap();
    write_file(
        &fixture,
        "Formula/codeinsight.rb",
        r#"
class Codeinsight < Formula
  desc "Local-first code intelligence MCP server for AI agents"

  def install
    bin.install "codeinsight"
  end
end
"#,
    );
    write_file(
        &fixture,
        "scripts/external-beta-trial.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

write_issue_body() {
  echo "external beta trial evidence flow"
}

write_redaction_checklist() {
  echo "redaction checklist"
}

main() {
  write_issue_body
  write_redaction_checklist
}

main "$@"
"#,
    );
    write_file(
        &fixture,
        "scripts/external-beta-trial-smoke.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "external beta trial evidence flow smoke"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);

    let outline = run_json([
        "outline",
        fixture
            .path()
            .join("scripts/external-beta-trial.sh")
            .to_str()
            .unwrap(),
    ]);
    assert!(
        outline
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| { symbol["name"] == "write_issue_body" && symbol["language"] == "bash" })
    );

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand external beta trial evidence flow",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(
        context["selected_seeds"][0]["value"], "scripts/external-beta-trial.sh",
        "script path and function text should beat the generic Codeinsight formula"
    );
    assert!(
        context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "external")
    );
    assert_eq!(
        context["files"][0]["file"],
        "scripts/external-beta-trial.sh"
    );
}

#[test]
fn cli_context_pack_uses_file_text_for_python_settings_tasks() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src/requests")).unwrap();
    std::fs::write(
        fixture.path().join("src/requests/__init__.py"),
        r#""""Requests package startup exports."""

from .sessions import Session, session
from .api import request

__all__ = ("Session", "session", "request")
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/requests/help.py"),
        r#""""Implementation metadata entrypoint."""

def _implementation():
    return "CPython"
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/requests/api.py"),
        r#""""User-facing request helpers."""

from . import sessions

def request(method, url, **kwargs):
    with sessions.Session() as session:
        return session.request(method=method, url=url, **kwargs)
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("src/requests/sessions.py"),
        r#""""Session objects manage persistent settings across requests."""

class Session:
    """Persistent session request flow."""

    def request(self, method, url, **kwargs):
        return method, url, kwargs

def merge_setting(request_setting, session_setting):
    """Merge request and session settings."""
    return request_setting or session_setting
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let settings_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand configuration settings",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(settings_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        settings_context["selected_seeds"][0]["value"],
        "src/requests/sessions.py"
    );
    assert!(
        settings_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "setting")
    );
    assert_eq!(
        settings_context["files"][0]["file"],
        "src/requests/sessions.py"
    );

    let session_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand requests session request flow",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(session_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        session_context["selected_seeds"][0]["value"],
        "src/requests/sessions.py"
    );
    assert_eq!(
        session_context["files"][0]["file"],
        "src/requests/sessions.py"
    );

    let startup_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand package startup flow",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(
        startup_context["selected_seeds"][0]["value"],
        "src/requests/__init__.py"
    );
    assert_eq!(
        startup_context["files"][0]["file"],
        "src/requests/__init__.py"
    );
}

#[test]
fn cli_context_pack_prefers_task_match_over_entrypoint_for_specific_routing_tasks() {
    let fixture = TempDir::new().unwrap();
    std::fs::write(
        fixture.path().join("go.mod"),
        "module github.com/gin-gonic/gin\n",
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("gin.go"),
        r#"package gin

// Engine is the framework instance and embeds the router group.
type Engine struct {
  RouterGroup
}

func New() *Engine {
  return &Engine{}
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("routergroup.go"),
        r#"package gin

// IRoutes defines router handle behavior.
type IRoutes interface {
  Handle(string, string, ...HandlerFunc) IRoutes
  GET(string, ...HandlerFunc) IRoutes
}

// RouterGroup defines gin engine routing behavior for route groups.
type RouterGroup struct {}

func (group *RouterGroup) Handle(method string, path string, handlers ...HandlerFunc) IRoutes {
  return group
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("context.go"),
        r#"package gin

type Context struct {
  Request *Request
}

type Request struct {
  Body string
}

type HandlerFunc func(*Context)

func Handler(c *Context) {}

func (c *Context) Bind(obj any) error {
  return c.ShouldBind(obj)
}

func (c *Context) ShouldBind(obj any) error {
  return nil
}

func (c *Context) Header(key string, value string) {}

func (c *Context) SetCookie(name string, value string) {}

func (c *Context) Redirect(code int, location string) {}
"#,
    )
    .unwrap();
    std::fs::create_dir_all(fixture.path().join("binding")).unwrap();
    std::fs::write(
        fixture.path().join("binding/binding.go"),
        r#"package binding

type BindingBody interface {
  BindBody([]byte, any) error
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("binding/default_validator.go"),
        r#"package binding

// defaultValidator applies binding validation rules after request binding.
type defaultValidator struct {}

func (v *defaultValidator) ValidateStruct(obj any) error {
  return nil
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("binding/default_validator_test.go"),
        r#"package binding

// TestDefaultValidator covers binding validation regressions.
func TestDefaultValidator(t any) {
  validator := defaultValidator{}
  _ = validator.ValidateStruct(nil)
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("binding/json.go"),
        r#"package binding

// jsonBinding deserializes JSON payloads during request binding.
type jsonBinding struct {}

func (jsonBinding) BindBody(body []byte, obj any) error {
  return nil
}
"#,
    )
    .unwrap();
    std::fs::create_dir_all(fixture.path().join("render")).unwrap();
    std::fs::write(
        fixture.path().join("render/render.go"),
        r#"package render

type Header map[string]string
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("render/redirect.go"),
        r#"package render

type Redirect struct {
  Location string
}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("response_writer.go"),
        r#"package gin

type ResponseWriter interface {
  Header() map[string]string
}
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let routing_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand gin engine routing behavior",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(routing_context["seed_strategy"], "auto_task_match");
    assert_eq!(
        routing_context["selected_seeds"][0]["value"],
        "routergroup.go"
    );
    assert_eq!(routing_context["files"][0]["file"], "routergroup.go");

    for task in [
        "understand gin request body parsing behavior",
        "understand gin redirect response behavior",
        "understand gin response header behavior",
        "understand gin response cookie behavior",
    ] {
        let operation_context = run_json([
            "context-pack",
            fixture.path().to_str().unwrap(),
            "--task",
            task,
            "--token-budget",
            "1600",
        ]);

        assert_eq!(operation_context["seed_strategy"], "auto_task_match");
        assert_eq!(
            operation_context["selected_seeds"][0]["value"], "context.go",
            "task should start from framework context: {task}"
        );
        assert_eq!(
            operation_context["files"][0]["file"], "context.go",
            "task should read framework context first: {task}"
        );
    }

    for (task, expected_file) in [
        (
            "understand binding validation behavior",
            "binding/default_validator.go",
        ),
        ("understand json binding behavior", "binding/json.go"),
        (
            "understand binding validation test coverage",
            "binding/default_validator_test.go",
        ),
    ] {
        let validation_context = run_json([
            "context-pack",
            fixture.path().to_str().unwrap(),
            "--task",
            task,
            "--token-budget",
            "1600",
        ]);

        assert_eq!(validation_context["seed_strategy"], "auto_task_match");
        assert_eq!(
            validation_context["selected_seeds"][0]["value"], expected_file,
            "task should start from the specific binding validation file: {task}"
        );
        assert_eq!(
            validation_context["files"][0]["file"], expected_file,
            "task should read the specific binding validation file first: {task}"
        );
    }

    let flask_fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(flask_fixture.path().join("src/flask/sansio")).unwrap();
    write_file(
        &flask_fixture,
        "src/flask/debughelpers.py",
        r#""""Debug helpers for routing redirects."""

from werkzeug.routing import RequestRedirect

class FormDataRoutingRedirect(RequestRedirect):
    pass
"#,
    );
    write_file(
        &flask_fixture,
        "src/flask/sansio/scaffold.py",
        r#""""Route registration helpers."""

class Scaffold:
    def route(self, rule, **options):
        return self.add_url_rule(rule, **options)

    def add_url_rule(self, rule, **options):
        return rule
"#,
    );
    write_file(
        &flask_fixture,
        "src/flask/ctx.py",
        r#""""Request context helpers."""

def after_this_request(f):
    return f
"#,
    );
    write_file(
        &flask_fixture,
        "src/flask/app.py",
        r#""""Flask request lifecycle."""

class Flask:
    def dispatch_request(self, ctx):
        return ctx.request

    def full_dispatch_request(self, ctx):
        response = self.preprocess_request(ctx)
        if response is None:
            response = self.dispatch_request(ctx)
        return self.finalize_request(ctx, response)

    def finalize_request(self, ctx, response):
        return self.process_response(ctx, response)

    def preprocess_request(self, ctx):
        return None

    def process_response(self, ctx, response):
        return response

    def do_teardown_request(self, ctx, exc):
        return exc
"#,
    );
    write_file(
        &flask_fixture,
        "src/flask/cli.py",
        r#"def main():
    return "cli"
"#,
    );

    run_json(["index", flask_fixture.path().to_str().unwrap(), "--force"]);

    let flask_routing_context = run_json([
        "context-pack",
        flask_fixture.path().to_str().unwrap(),
        "--task",
        "understand flask application routing behavior",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(
        flask_routing_context["selected_seeds"][0]["value"],
        "src/flask/sansio/scaffold.py"
    );
    assert_eq!(
        flask_routing_context["files"][0]["file"],
        "src/flask/sansio/scaffold.py"
    );

    let django_fixture = TempDir::new().unwrap();
    write_file(
        &django_fixture,
        "django/urls/resolvers.py",
        r#""""URL resolving and routing behavior."""

class URLResolver:
    def resolve(self, path):
        return ResolverMatch(path)

class URLPattern:
    def match(self, path):
        return path

class ResolverMatch:
    def __init__(self, path):
        self.path = path
"#,
    );
    write_file(
        &django_fixture,
        "django/core/checks/urls.py",
        r#""""System checks for URL configuration."""

def check_url_config(app_configs, **kwargs):
    return []

def check_url_namespaces_unique(app_configs, **kwargs):
    return []
"#,
    );
    write_file(
        &django_fixture,
        "django/core/handlers/base.py",
        r#""""Request and response lifecycle."""

class BaseHandler:
    def get_response(self, request):
        return self._get_response(request)

    def load_middleware(self):
        return []
"#,
    );

    run_json(["index", django_fixture.path().to_str().unwrap(), "--force"]);

    let django_routing_context = run_json([
        "context-pack",
        django_fixture.path().to_str().unwrap(),
        "--task",
        "understand django URL routing behavior",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(
        django_routing_context["selected_seeds"][0]["value"],
        "django/urls/resolvers.py"
    );
    assert_eq!(
        django_routing_context["files"][0]["file"],
        "django/urls/resolvers.py"
    );

    let flask_lifecycle_context = run_json([
        "context-pack",
        flask_fixture.path().to_str().unwrap(),
        "--task",
        "understand request lifecycle before after request handling",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(
        flask_lifecycle_context["selected_seeds"][0]["value"],
        "src/flask/app.py"
    );
    assert_eq!(
        flask_lifecycle_context["files"][0]["file"],
        "src/flask/app.py"
    );
    let flask_lifecycle_selection_reason = flask_lifecycle_context["files"][0]["reason"]
        .as_str()
        .unwrap();
    assert!(flask_lifecycle_selection_reason.contains("request lifecycle task"));
    assert!(flask_lifecycle_selection_reason.contains("framework handler or app seed file"));
    assert!(
        flask_lifecycle_context["reading_plan"][0]["selection_reason"]
            .as_str()
            .unwrap()
            .contains("request lifecycle task")
    );
    let flask_lifecycle_focus = flask_lifecycle_context["reading_plan"][0]["focus"]
        .as_str()
        .unwrap();
    assert!(flask_lifecycle_focus.contains("request lifecycle"));
    assert!(flask_lifecycle_focus.contains("response finalization"));
    let flask_lifecycle_question = flask_lifecycle_context["reading_plan"][0]["question"]
        .as_str()
        .unwrap();
    assert!(flask_lifecycle_question.contains("request lifecycle"));
    assert!(flask_lifecycle_question.contains("dispatch"));
    assert!(flask_lifecycle_question.contains("response finalization"));
}

#[test]
fn cli_context_pack_prefers_startup_entrypoint_over_generic_server_route() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("lib/streamlit/web/server/starlette")).unwrap();
    std::fs::write(
        fixture.path().join("lib/streamlit/web/bootstrap.py"),
        r#""""Streamlit web bootstrap entrypoint."""

from streamlit.web.server.server import Server

def main():
    """Start Streamlit's web server."""
    return Server().start()
"#,
    )
    .unwrap();
    std::fs::write(
        fixture
            .path()
            .join("lib/streamlit/web/server/starlette/starlette_auth_routes.py"),
        r#""""Starlette auth routes for the Streamlit server."""

def register_auth_routes(app):
    return app
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("lib/streamlit/web/server/server.py"),
        r#"class Server:
    def start(self):
        return "started"
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let startup_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand streamlit server startup flow",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(
        startup_context["selected_seeds"][0]["value"],
        "lib/streamlit/web/bootstrap.py"
    );
    assert_eq!(
        startup_context["files"][0]["file"],
        "lib/streamlit/web/bootstrap.py"
    );
}

#[test]
fn cli_context_pack_prefers_core_config_over_ui_settings() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("lib/streamlit")).unwrap();
    std::fs::create_dir_all(
        fixture
            .path()
            .join("frontend/app/src/components/StreamlitDialog"),
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("lib/streamlit/config.py"),
        r#""""Core configuration settings for Streamlit."""

def get_config_options():
    return {"server.port": 8501}
"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("lib/streamlit/config_util.py"),
        r#""""Helpers for displaying configuration settings."""

def show_config(config_options):
    return list(config_options)
"#,
    )
    .unwrap();
    std::fs::write(
        fixture
            .path()
            .join("frontend/app/src/components/StreamlitDialog/UserSettings.ts"),
        r#"export function UserSettings() {
  return "settings";
}
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let settings_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand configuration settings",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(
        settings_context["selected_seeds"][0]["value"],
        "lib/streamlit/config.py"
    );
    assert_eq!(
        settings_context["files"][0]["file"],
        "lib/streamlit/config.py"
    );
}

#[test]
fn cli_context_pack_uses_task_specific_seed_file_question() {
    let fixture = TempDir::new().unwrap();
    std::fs::create_dir_all(fixture.path().join("src")).unwrap();
    std::fs::write(
        fixture.path().join("src/flow.py"),
        r#"
def leaf():
    return "ok"

def service():
    return leaf()
"#,
    )
    .unwrap();

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand impact call path behavior",
        "--file",
        "src/flow.py",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(context["reading_plan"][0]["file"], "src/flow.py");
    assert_eq!(
        context["reading_plan"][0]["next_action"],
        "inspect_seed_file"
    );
    let question = context["reading_plan"][0]["question"].as_str().unwrap();
    assert!(question.contains("callers"));
    assert!(question.contains("impact paths"));
}

#[test]
fn cli_context_pack_uses_task_specific_reference_question() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/session.ts",
        r#"
import { AUTH_TOKEN } from "./tokens";

export const sessionHeader = AUTH_TOKEN;
"#,
    );
    write_file(
        &fixture,
        "src/tokens.ts",
        r#"
export const AUTH_TOKEN = "x-session";
"#,
    );

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand authentication session token usage",
        "--symbol",
        "AUTH_TOKEN",
        "--token-budget",
        "1600",
    ]);
    let reference_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/session.ts")
        .unwrap();
    assert_eq!(reference_step["next_action"], "inspect_references");
    assert_eq!(reference_step["suggested_tool"]["tool"], "impact_analysis");
    assert_eq!(
        reference_step["suggested_tool"]["suggested_arguments"]["files"][0],
        "src/session.ts"
    );
    assert_eq!(
        reference_step["suggested_tool"]["suggested_arguments"]["format"],
        "summary"
    );
    assert_eq!(
        reference_step["suggested_tool"]["suggested_arguments"]["depth"].as_u64(),
        Some(2)
    );
    let question = reference_step["question"].as_str().unwrap();
    assert!(question.contains("authentication decisions"));
    assert!(question.contains("session state"));
}

#[test]
fn cli_resolves_pnpm_workspace_package_exports() {
    let fixture = pnpm_workspace_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 10);
    assert_eq!(index["changed_files"], 10);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "pnpm-ui/button"
                    && dependency["resolved_file"] == "packages/pnpm-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "version-star-ui/button"
                    && dependency["resolved_file"] == "packages/version-star-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "version-caret-ui/button"
                    && dependency["resolved_file"] == "packages/version-caret-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "version-tilde-ui/button"
                    && dependency["resolved_file"] == "packages/version-tilde-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "version-exact-ui/button"
                    && dependency["resolved_file"] == "packages/version-exact-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "deep-ui/button"
                    && dependency["resolved_file"] == "packages/nested/deep-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "catalog-ui/button"
                    && dependency["resolved_file"] == "node_modules/catalog-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "default-catalog-ui/button"
                    && dependency["resolved_file"]
                        == "node_modules/default-catalog-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "legacy-ui/button"
                    && dependency["resolved_file"] == "node_modules/legacy-ui/dist/button.js"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "pnpmWorkspaceMain",
        "--limit",
        "5",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "pnpmButton" && call["callee_file"] == "packages/pnpm-ui/src/button.ts"
    }));

    let version_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "pnpmWorkspaceVersionMain",
        "--limit",
        "12",
    ]);
    assert!(version_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "versionStarButton"
            && call["callee_file"] == "packages/version-star-ui/src/button.ts"
    }));
    assert!(version_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "versionCaretButton"
            && call["callee_file"] == "packages/version-caret-ui/src/button.ts"
    }));
    assert!(version_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "versionTildeButton"
            && call["callee_file"] == "packages/version-tilde-ui/src/button.ts"
    }));
    assert!(version_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "versionExactButton"
            && call["callee_file"] == "packages/version-exact-ui/src/button.ts"
    }));
    assert!(version_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "deepButton"
            && call["callee_file"] == "packages/nested/deep-ui/src/button.ts"
    }));
    assert!(version_callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "legacyButton"
            || call["callee_file"] != "packages/legacy/legacy-ui/src/button.ts"
    }));
}

#[test]
fn cli_resolves_workspace_protocol_package_exports() {
    let fixture = workspace_protocol_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);
    assert_eq!(index["changed_files"], 6);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "protocol-ui/button"
                    && dependency["resolved_file"] == "packages/protocol-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "protocol-ui/feature/special"
                    && dependency["resolved_file"] == "packages/protocol-ui/src/feature-special.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "protocol-ui/feature/special/button"
                    && dependency["resolved_file"]
                        == "packages/protocol-ui/src/feature-special/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "protocol-ui/feature/special"
                    || dependency["resolved_file"] != "packages/protocol-ui/src/feature/special.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "workspaceProtocolMain",
        "--limit",
        "8",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "protocolButton"
            && call["callee_file"] == "packages/protocol-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "protocolSpecial"
            && call["callee_file"] == "packages/protocol-ui/src/feature-special.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "protocolSpecialButton"
            && call["callee_file"] == "packages/protocol-ui/src/feature-special/button.ts"
    }));
}

#[test]
fn cli_respects_null_package_exports_without_subpath_fallback() {
    let fixture = null_package_exports_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 7);
    assert_eq!(index["changed_files"], 7);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "null-export-lib/enabled"
                    && dependency["resolved_file"] == "src/enabled.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "null-export-lib/disabled"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "null-export-lib/array"
                    && dependency["resolved_file"] == "src/array-fallback.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "null-export-lib/conditional"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "null-export-lib/conditional-external"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "null-export-lib/disabled"
                    || dependency["resolved_file"] != "src/disabled.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "null-export-lib/conditional"
                    || dependency["resolved_file"] != "src/conditional-fallback.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "null-export-lib/conditional-external"
                    || dependency["resolved_file"] != "src/conditional-external-fallback.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "null-export-lib/conditional-external"
                    || dependency["resolved_file"] != "external-export-lib.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "null-export-lib/array"
                    || dependency["resolved_file"] != "external-export-lib.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "nullExportMain",
        "--limit",
        "6",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "enabledRender" && call["callee_file"] == "src/enabled.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "arrayRender" && call["callee_file"] == "src/array-fallback.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "disabledRender" || call["callee_file"] != "src/disabled.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalRender"
            || call["callee_file"] != "src/conditional-fallback.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalExternalRender"
            || call["callee_file"] != "src/conditional-external-fallback.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalExternalRender"
            || call["callee_file"] != "external-export-lib.ts"
    }));
}

#[test]
fn cli_resolves_package_subpath_metadata_fallbacks() {
    let fixture = package_subpath_fallback_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 1);
    assert_eq!(index["changed_files"], 1);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "30",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "subpath-fallback-lib/file"
                    && dependency["resolved_file"] == "node_modules/subpath-fallback-lib/file.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "subpath-fallback-lib/dir"
                    && dependency["resolved_file"]
                        == "node_modules/subpath-fallback-lib/dir/index.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "extensionless-export-lib/feature"
                    && dependency["resolved_file"]
                        == "node_modules/extensionless-export-lib/dist/feature.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "wildcard-precedence-lib/feature/special"
                    && dependency["resolved_file"]
                        == "node_modules/wildcard-precedence-lib/dist/special.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "wildcard-precedence-lib/feature/special/button"
                    && dependency["resolved_file"]
                        == "node_modules/wildcard-precedence-lib/dist/special/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "wildcard-precedence-lib/feature/special"
                    || dependency["resolved_file"]
                        != "node_modules/wildcard-precedence-lib/dist/wildcard/special.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "subpath-fallback-lib/missing"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "subpath-disabled-lib/disabled"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "subpath-disabled-lib/disabled"
                    || dependency["resolved_file"]
                        != "node_modules/subpath-disabled-lib/disabled.js"
            })
    );
}

#[test]
fn cli_resolves_c_like_local_includes() {
    let fixture = c_like_include_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 5);
    assert_eq!(index["changed_files"], 5);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "auth.h" && dependency["resolved_file"] == "src/auth.h"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "include/shared.hpp"
                    && dependency["resolved_file"] == "include/shared.hpp"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "../include/shared.hpp"
                    && dependency["resolved_file"] == "include/shared.hpp"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "<stdio.h>" && dependency["resolved_file"].is_null()
            })
    );
    assert_eq!(deps["summary"]["edges"].as_u64(), Some(4));
    assert_eq!(deps["summary"]["resolved_edges"].as_u64(), Some(3));
    assert_eq!(deps["summary"]["local_edges"].as_u64(), Some(3));
    assert_eq!(deps["summary"]["unresolved_edges"].as_u64(), Some(1));
    assert_eq!(deps["summary"]["external_targets"].as_u64(), Some(1));
    assert!(
        deps["summary"]["top_external_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["target"] == "<stdio.h>" && target["edges"] == 1)
    );
    assert!(
        deps["top_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source_file"] == "src/auth.c" && source["edges"] == 2)
    );
    assert!(
        deps["top_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["target"] == "include/shared.hpp" && target["edges"] == 1)
    );

    let c_deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--language",
        "c",
        "--limit",
        "20",
    ]);
    assert!(
        c_deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| dependency["language"] == "c")
    );
    assert!(
        c_deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| dependency["target"] == "auth.h")
    );
    assert!(
        c_deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| dependency["target"] != "include/shared.hpp")
    );

    let source_file_deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--file",
        "src/service.cpp",
        "--limit",
        "20",
    ]);
    assert_eq!(source_file_deps["edges"].as_u64(), Some(1));
    assert_eq!(
        source_file_deps["dependencies"][0]["source_file"],
        "src/service.cpp"
    );
    assert_eq!(
        source_file_deps["dependencies"][0]["resolved_file"],
        "include/shared.hpp"
    );

    let target_file_deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--file",
        "include/shared.hpp",
        "--language",
        "c++",
        "--limit",
        "20",
    ]);
    assert_eq!(target_file_deps["edges"].as_u64(), Some(2));
    assert_eq!(target_file_deps["summary"]["edges"].as_u64(), Some(2));
    assert_eq!(
        target_file_deps["summary"]["resolved_edges"].as_u64(),
        Some(2)
    );
    assert!(
        target_file_deps["top_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["target"] == "include/shared.hpp" && target["edges"] == 1)
    );
    assert!(
        target_file_deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["language"] == "cpp"
                    && dependency["resolved_file"] == "include/shared.hpp"
            })
    );

    let service_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "service",
        "--limit",
        "10",
    ]);
    assert!(service_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "shared_value" && call["callee_file"] == "include/shared.hpp"
    }));

    let client_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "client",
        "--limit",
        "10",
    ]);
    assert!(client_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "shared_value" && call["callee_file"] == "include/shared.hpp"
    }));
    assert!(client_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "declared_value" && call["callee_file"] == "include/shared.hpp"
    }));
}

#[test]
fn cli_resolves_go_module_imports() {
    let fixture = go_module_import_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 5);
    assert_eq!(index["changed_files"], 5);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "github.com/example/codeinsight/internal/auth"
                    && dependency["resolved_file"] == "internal/auth/service.go"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "github.com/example/codeinsight/internal/config"
                    && dependency["resolved_file"] == "internal/config/config.go"
                    && dependency["local_alias"] == "cfg"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "github.com/example/codeinsight/internal/metrics"
                    && dependency["resolved_file"] == "internal/metrics/metrics.go"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "github.com/example/codeinsight/internal/metrics"
                    || dependency["resolved_file"] != "internal/metrics/doc.go"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "fmt" && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "github.com/acme/remote"
                    && dependency["resolved_file"].is_null()
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "main",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "auth.Login" && call["callee_file"] == "internal/auth/service.go"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "cfg.Load" && call["callee_file"] == "internal/config/config.go"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "metrics.Track" && call["callee_file"] == "internal/metrics/metrics.go"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee"] != "remote.Name" || call["callee_file"].is_null() })
    );
}

#[test]
fn cli_resolves_java_source_imports() {
    let fixture = java_source_import_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);
    assert_eq!(index["changed_files"], 6);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "com.example.auth.AuthService"
                    && dependency["resolved_file"]
                        == "src/main/java/com/example/auth/AuthService.java"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "com.example.util.Names.defaultName"
                    && dependency["resolved_file"] == "src/main/java/com/example/util/Names.java"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "java.util.List" && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "com.example.reporting.*"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "com.acme.RemoteClient"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "BaseApp"
                    && dependency["kind"] == "base_type"
                    && dependency["local_alias"] == "App"
                    && dependency["imported_symbol"] == "extends"
            })
    );
    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "App.run",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "AuthService.login"
            && call["callee_file"] == "src/main/java/com/example/auth/AuthService.java"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "defaultName"
            && call["callee_file"] == "src/main/java/com/example/util/Names.java"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "Report.log"
            && call["callee_file"] == "src/main/java/com/example/reporting/Report.java"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "LocalFormatter.decorate"
            && call["callee_file"] == "src/main/java/com/example/app/LocalFormatter.java"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee"] != "remote.id" || call["callee_file"].is_null() })
    );
}

#[test]
fn cli_context_pack_routes_java_base_type_relations() {
    let fixture = java_source_import_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand inherited application startup behavior",
        "--file",
        "src/main/java/com/example/app/App.java",
        "--token-budget",
        "5000",
    ]);

    let base_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/main/java/com/example/app/BaseApp.java")
        .expect("java base type should be selected through type relation evidence");
    assert!(
        base_file["source_mix"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source"] == "type relation")
    );
    let base_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/main/java/com/example/app/BaseApp.java")
        .expect("java base type should have a reading step");
    assert_eq!(base_step["next_action"], "inspect_type_relation");
    assert_eq!(base_step["suggested_tool"]["tool"], "dependency_graph");
    assert_eq!(
        base_step["suggested_tool"]["suggested_arguments"]["kinds"][0],
        "base_type"
    );
}

#[test]
fn cli_context_pack_routes_typescript_base_type_relations() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/auth.ts",
        r#"
export interface AuthContract {}

export class AuthController extends BaseController implements AuthContract {
  login() {
    return this.session();
  }
}
"#,
    );
    write_file(
        &fixture,
        "src/base.ts",
        r#"
export class BaseController {
  session() {
    return "active";
  }
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand inherited authentication controller behavior",
        "--file",
        "src/auth.ts",
        "--token-budget",
        "4000",
    ]);

    let base_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/base.ts")
        .expect("typescript base type should be selected through type relation evidence");
    assert!(base_file["ranges"].as_array().unwrap().iter().any(|range| {
        range["source"] == "type_relation"
            && range["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("base type BaseController"))
    }));
    let base_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/base.ts")
        .expect("typescript base type should have a reading step");
    assert_eq!(base_step["next_action"], "inspect_type_relation");
    assert_eq!(base_step["suggested_tool"]["tool"], "dependency_graph");
    assert_eq!(
        base_step["suggested_tool"]["suggested_arguments"]["kinds"][0],
        "base_type"
    );
}

#[test]
fn cli_context_pack_routes_php_base_type_relations() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/AuthController.php",
        r#"<?php
class AuthController extends BaseController implements AuthGuard {
    public function login(): string {
        return $this->session();
    }
}
"#,
    );
    write_file(
        &fixture,
        "src/BaseController.php",
        r#"<?php
class BaseController {
    protected function session(): string {
        return "active";
    }
}
"#,
    );
    write_file(
        &fixture,
        "src/AuthGuard.php",
        r#"<?php
interface AuthGuard {
    public function login(): string;
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);

    let graph = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--kind",
        "base-type",
        "--limit",
        "10",
    ]);
    assert_eq!(graph["edges"].as_u64(), Some(2));
    assert!(
        graph["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| dependency["kind"] == "base_type")
    );

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand inherited php authentication controller behavior",
        "--file",
        "src/AuthController.php",
        "--token-budget",
        "4000",
    ]);

    let base_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/BaseController.php")
        .expect("php base type should be selected through type relation evidence");
    assert!(
        base_file["source_mix"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source"] == "type relation")
    );
    assert!(base_file["ranges"].as_array().unwrap().iter().any(|range| {
        range["source"] == "type_relation"
            && range["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("base type BaseController"))
    }));
    let base_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/BaseController.php")
        .expect("php base type should have a reading step");
    assert_eq!(base_step["next_action"], "inspect_type_relation");
    assert_eq!(base_step["suggested_tool"]["tool"], "dependency_graph");
}

#[test]
fn cli_context_pack_routes_ruby_base_type_relations() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "lib/auth_service.rb",
        r#"
class AuthService < BaseService
  def login
    session
  end
end
"#,
    );
    write_file(
        &fixture,
        "lib/base_service.rb",
        r#"
class BaseService
  def session
    "active"
  end
end
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let graph = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--kind",
        "base_type",
        "--limit",
        "10",
    ]);
    assert_eq!(graph["edges"].as_u64(), Some(1));
    assert!(
        graph["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| dependency["kind"] == "base_type")
    );

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand inherited ruby authentication service behavior",
        "--file",
        "lib/auth_service.rb",
        "--token-budget",
        "3000",
    ]);

    let base_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "lib/base_service.rb")
        .expect("ruby base type should be selected through type relation evidence");
    assert!(
        base_file["source_mix"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source"] == "type relation")
    );
    assert!(base_file["ranges"].as_array().unwrap().iter().any(|range| {
        range["source"] == "type_relation"
            && range["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("base type BaseService"))
    }));
    let base_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "lib/base_service.rb")
        .expect("ruby base type should have a reading step");
    assert_eq!(base_step["next_action"], "inspect_type_relation");
    assert_eq!(base_step["suggested_tool"]["tool"], "dependency_graph");
}

#[test]
fn cli_resolves_php_namespace_use_imports() {
    let fixture = php_namespace_use_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);
    assert_eq!(index["changed_files"], 6);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App\\Repository\\UserRepository"
                    && dependency["resolved_file"] == "src/Repository/UserRepository.php"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App\\Support\\AuditLog"
                    && dependency["resolved_file"] == "src/Support/AuditLog.php"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App\\Support\\audit_login"
                    && dependency["resolved_file"] == "src/Support/audit_login.php"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App\\Support\\Metrics"
                    && dependency["resolved_file"] == "src/Support/Metrics.php"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App\\Support\\audit_event"
                    && dependency["resolved_file"] == "src/Support/audit_event.php"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "Vendor\\Package\\RemoteClient"
                    && dependency["resolved_file"].is_null()
            })
    );
    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthController.login",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "AuditLog.record" && call["callee_file"] == "src/Support/AuditLog.php"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "audit_login" && call["callee_file"] == "src/Support/audit_login.php"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "MetricsAlias.track" && call["callee_file"] == "src/Support/Metrics.php"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "event" && call["callee_file"] == "src/Support/audit_event.php"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee"] != "id" || call["callee_file"].is_null() })
    );
}

#[test]
fn cli_resolves_ruby_require_relative_imports() {
    let fixture = ruby_require_relative_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "support/audit"
                    && dependency["resolved_file"] == "lib/support/audit.rb"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "../support/audit.rb"
                    && dependency["resolved_file"] == "lib/support/audit.rb"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "json" && dependency["resolved_file"].is_null()
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "Example.AuthService.login",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "Audit.record" && call["callee_file"] == "lib/support/audit.rb"
    }));
    let nested_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "Example.Services.Runner.run",
        "--limit",
        "10",
    ]);
    assert!(nested_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "Audit.record" && call["callee_file"] == "lib/support/audit.rb"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee"] != "JSON.generate" || call["callee_file"].is_null() })
    );
}

#[test]
fn cli_resolves_csharp_using_imports() {
    let fixture = csharp_using_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 15);
    assert_eq!(index["changed_files"], 15);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let base_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "BaseTag",
        "--limit",
        "5",
    ]);
    assert!(base_symbols.as_array().unwrap().iter().any(|symbol| {
        symbol["name"] == "BaseTag" && symbol["file"] == "src/App/Controllers/BaseController.cs"
    }));
    let find_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "Find",
        "--limit",
        "20",
    ]);
    assert!(
        find_symbols
            .as_array()
            .unwrap()
            .iter()
            .filter(|symbol| {
                symbol["name"] == "Find" && symbol["file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 2
    );
    let interface_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "IUserDirectory",
        "--limit",
        "5",
    ]);
    assert!(interface_symbols.as_array().unwrap().iter().any(|symbol| {
        symbol["name"] == "IUserDirectory"
            && symbol["file"] == "src/App/Contracts/IUserDirectory.cs"
    }));
    let extension_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "FormatForDisplay",
        "--limit",
        "5",
    ]);
    assert!(extension_symbols.as_array().unwrap().iter().any(|symbol| {
        symbol["name"] == "FormatForDisplay"
            && symbol["file"] == "src/App/Extensions/UserServiceExtensions.cs"
    }));
    let profile_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "ProfileService",
        "--limit",
        "5",
    ]);
    assert!(profile_symbols.as_array().unwrap().iter().any(|symbol| {
        symbol["name"] == "ProfileService" && symbol["file"] == "src/App/Services/UserService.cs"
    }));
    let load_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "Load",
        "--limit",
        "5",
    ]);
    assert!(load_symbols.as_array().unwrap().iter().any(|symbol| {
        symbol["name"] == "Load" && symbol["file"] == "src/App/Services/UserService.cs"
    }));
    let external_profile_symbols = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "ExternalProfile",
        "--limit",
        "5",
    ]);
    assert!(
        external_profile_symbols
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| {
                symbol["name"] == "ExternalProfile"
                    && symbol["file"] == "src/App/Profiles/ExternalProfile.cs"
            })
    );

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "120",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Controllers.BaseController"
                    && dependency["kind"] == "base_type"
                    && dependency["local_alias"] == "AuthController"
                    && dependency["imported_symbol"] == "base"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "ExternalProfile"
                    && dependency["kind"] == "property_type"
                    && dependency["local_alias"] == "ExternalProfile"
                    && dependency["imported_symbol"] == "UserService"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Profiles.ExternalProfile"
                    && dependency["kind"] == "property_type"
                    && dependency["local_alias"] == "QualifiedExternalProfile"
                    && dependency["imported_symbol"] == "UserService"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "IAuthController"
                    && dependency["kind"] == "base_type"
                    && dependency["local_alias"] == "AuthController"
                    && dependency["imported_symbol"] == "implements"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Services"
                    && dependency["resolved_file"] == "src/App/Services/UserService.cs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Support.AuditLog"
                    && dependency["resolved_file"] == "src/App/Support/AuditLog.cs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Support.MathUtil"
                    && dependency["resolved_file"] == "src/App/Support/MathUtil.cs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Extensions"
                    && dependency["resolved_file"] == "src/App/Extensions/UserServiceExtensions.cs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Contracts"
                    && dependency["resolved_file"] == "src/App/Contracts/IAuthController.cs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "System" && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "IUserDirectory"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "directory"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Controllers"
                    && dependency["kind"] == "namespace"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "extension_method"
                    && dependency["local_alias"] == "FormatForDisplay"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "ProfileService"
                    && dependency["kind"] == "property_type"
                    && dependency["local_alias"] == "Profile"
                    && dependency["imported_symbol"] == "UserService"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "users"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Services.UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "backupUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "Repo"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "repoUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "createdUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "App.Services.UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "createdBackupUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "targetUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "maybeUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "servicePool"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "listUsers"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "usersById"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "lazyUsers"
                    && dependency["imported_symbol"] == "Value"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "inferredLazyUsers"
                    && dependency["imported_symbol"] == "Value"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "taskUsers"
                    && dependency["imported_symbol"] == "Result"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "valueTaskUsers"
                    && dependency["imported_symbol"] == "Result"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "inferredTaskUsers"
                    && dependency["imported_symbol"] == "Result"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "UserService"
                    && dependency["kind"] == "type_binding"
                    && dependency["local_alias"] == "inferredValueTaskUsers"
                    && dependency["imported_symbol"] == "Result"
                    && dependency["resolved_file"].is_null()
            })
    );
    {
        let local_alias = "nestedUsers";
        assert!(
            deps["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .all(|dependency| {
                    dependency["local_alias"] != local_alias || dependency["kind"] != "type_binding"
                })
        );
    }
    for (local_alias, wrapper_member) in [
        ("taskListUsers", "Result"),
        ("lazyMappedUsers", "Value"),
        ("inferredTaskListUsers", "Result"),
        ("inferredLazyMappedUsers", "Value"),
    ] {
        assert!(
            deps["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .any(|dependency| {
                    dependency["target"] == "UserService"
                        && dependency["kind"] == "type_binding"
                        && dependency["local_alias"] == local_alias
                        && dependency["imported_symbol"] == wrapper_member
                        && dependency["resolved_file"].is_null()
                })
        );
    }

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthController.Login",
        "--limit",
        "160",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "Audit.Record" && call["callee_file"] == "src/App/Support/AuditLog.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "Audit.Record" || call["callee_file"] != "src/App/Controllers/Audit.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "App.Support.AuditLog.Record"
            && call["callee_file"] == "src/App/Support/AuditLog.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "ClampName" && call["callee_file"] == "src/App/Support/MathUtil.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "ClampName" || call["callee_file"] != "src/App/Conflicts/A.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "App.Support.MathUtil.ClampName"
            && call["callee_file"] == "src/App/Support/MathUtil.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "UserService.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "UserService.Find"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 8
    );
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "App.Services.UserService.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "UserService.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "UserService.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 3
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "App.Services.UserService.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 3
    );
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "LocalFormatter.Normalize"
            && call["callee_file"] == "src/App/Controllers/LocalFormatter.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "LocalFormatter.Normalize"
            || call["callee_file"] != "src/App/Conflicts/LocalFormatter.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "users.Find" && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "backupUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "repoUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "createdUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "createdBackupUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "targetUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "maybeUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "servicePool.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "listUsers.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "usersById.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "directory.Find"
            && call["callee_file"] == "src/App/Contracts/IUserDirectory.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "directory.Find"
            || call["callee_file"] != "src/App/Implementations/UserDirectory.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "directory.ImplementationProfile.Load" && call["callee_file"].is_null()
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "directory.ImplementationProfile.Load"
            || call["callee_file"] != "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "lazyUsers.Value.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredLazyUsers.Value.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "taskUsers.Result.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "valueTaskUsers.Result.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredTaskUsers.Result.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredValueTaskUsers.Result.Find"
            && call["callee_file"] == "src/App/Services/UserService.cs"
    }));
    for resolved_callee in [
        "taskListUsers.Result.Find",
        "lazyMappedUsers.Value.Find",
        "inferredTaskListUsers.Result.Find",
        "inferredLazyMappedUsers.Value.Find",
    ] {
        assert!(callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == resolved_callee
                && call["callee_file"] == "src/App/Services/UserService.cs"
        }));
    }
    for resolved_callee in [
        "taskListUsers.Result.ExternalProfile.Load",
        "lazyMappedUsers.Value.ExternalProfile.Load",
        "inferredTaskListUsers.Result.ExternalProfile.Load",
        "inferredLazyMappedUsers.Value.ExternalProfile.Load",
    ] {
        assert!(callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == resolved_callee
                && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
        }));
    }
    for (unresolved_callee, expected_count) in [
        ("users.FormatForDisplay", 2),
        ("maybeUsers.FormatForDisplay", 2),
        ("listUsers.FormatForDisplay", 1),
    ] {
        assert!(
            callees
                .as_array()
                .unwrap()
                .iter()
                .filter(|call| {
                    call["callee"] == unresolved_callee
                        && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
                })
                .count()
                >= expected_count
        );
    }
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "users.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "users.QualifiedExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "backupUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "backupUsers.QualifiedExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "repoUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "createdUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "createdBackupUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "targetUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "lazyUsers.Value.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredLazyUsers.Value.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "taskUsers.Result.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "valueTaskUsers.Result.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredTaskUsers.Result.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "inferredValueTaskUsers.Result.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "maybeUsers.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 2
    );
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "servicePool.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "listUsers.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "usersById.ExternalProfile.Load"
            && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 2
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "backupUsers.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 2
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "repoUsers.ExternalProfile.Load"
                    && call["callee_file"] == "src/App/Profiles/ExternalProfile.cs"
            })
            .count()
            >= 2
    );
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "users.Profile.Metadata.Display" && call["callee_file"].is_null()
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.Find"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 10
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.FindAsync"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 2
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.FindAs"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 3
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "repoUsers.Find"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 2
    );

    let local_callers = run_json([
        "callers",
        fixture.path().to_str().unwrap(),
        "LocalTag",
        "--limit",
        "5",
    ]);
    assert!(
        local_callers.as_array().unwrap().iter().any(|call| {
            call["caller"] == "AuthController.Login" && call["callee"] == "LocalTag"
        })
    );
    assert!(
        local_callers
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["caller"] == "AuthController.Login" && call["callee"] == "LocalTag"
            })
            .count()
            >= 2
    );
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "base.BaseTag"
            && call["callee_file"] == "src/App/Controllers/BaseController.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "this.BaseTag"
            || call["callee_file"] != "src/App/Controllers/BaseController.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "base.RootTag"
            && call["callee_file"] == "src/App/Controllers/RootController.cs"
    }));
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.Profile.Load"
                    && call["callee_file"] == "src/App/Services/UserService.cs"
            })
            .count()
            >= 2
    );
}

#[test]
fn cli_resolves_csharp_unqualified_base_inherited_calls() {
    let fixture = csharp_unqualified_base_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthController.Login",
        "--limit",
        "10",
    ]);

    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "base.BaseTag"
            && call["callee_file"] == "src/App/Controllers/BaseController.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "base.RootTag"
            && call["callee_file"] == "src/App/Controllers/RootController.cs"
    }));
}

#[test]
fn cli_leaves_csharp_nested_temporary_wrappers_unresolved() {
    let fixture = csharp_nested_temporary_wrapper_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthController.Login",
        "--limit",
        "20",
    ]);
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee_file"] != "src/App/Services/UserService.cs"
            && call["callee_file"] != "src/App/Profiles/ExternalProfile.cs"
    }));
}

#[test]
fn cli_leaves_csharp_extension_method_boundaries_unresolved() {
    let fixture = csharp_extension_method_boundary_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 10);
    assert_eq!(index["changed_files"], 10);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let missing_import_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "MissingImportController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        missing_import_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "users.FormatForDisplay" && call["callee_file"].is_null()
            })
    );
    assert!(
        missing_import_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| {
                call["callee"] != "users.FormatForDisplay"
                    || call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs"
            })
    );

    let wrong_receiver_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "WrongReceiverController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        wrong_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "product.FormatForDisplay" && call["callee_file"].is_null()
            })
    );
    assert!(
        wrong_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| {
                call["callee"] != "product.FormatForDisplay"
                    || call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs"
            })
    );

    let missing_import_temporary_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "MissingImportTemporaryController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        missing_import_temporary_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "UserService.FormatForDisplay" && call["callee_file"].is_null()
            })
    );
    assert!(
        missing_import_temporary_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| {
                call["callee"] != "UserService.FormatForDisplay"
                    || call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs"
            })
    );

    let wrong_temporary_receiver_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "WrongTemporaryReceiverController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        wrong_temporary_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .any(|call| {
                call["callee"] == "ProductService.FormatForDisplay" && call["callee_file"].is_null()
            })
    );
    assert!(
        wrong_temporary_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| {
                call["callee"] != "ProductService.FormatForDisplay"
                    || call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs"
            })
    );

    let nested_temporary_receiver_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "NestedTemporaryReceiverController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        nested_temporary_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs" })
    );

    let missing_import_qualified_temporary_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "MissingImportQualifiedTemporaryController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        missing_import_qualified_temporary_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs" })
    );

    let wrong_qualified_temporary_receiver_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "WrongQualifiedTemporaryReceiverController.Login",
        "--limit",
        "10",
    ]);
    assert!(
        wrong_qualified_temporary_receiver_callees
            .as_array()
            .unwrap()
            .iter()
            .all(|call| { call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs" })
    );
}

#[test]
fn cli_keeps_csharp_static_using_and_extension_methods_distinct() {
    let fixture = csharp_static_using_extension_conflict_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);
    assert_eq!(index["changed_files"], 4);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "ConflictController.Login",
        "--limit",
        "20",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "FormatForDisplay"
            && call["callee_file"] == "src/App/Support/DisplayFormatters.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "FormatForDisplay"
            || call["callee_file"] != "src/App/Extensions/UserServiceExtensions.cs"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "users.FormatForDisplay"
            && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "users.FormatForDisplay"
            || call["callee_file"] != "src/App/Support/DisplayFormatters.cs"
    }));
}

#[test]
fn cli_resolves_csharp_extension_method_receiver_variants() {
    let fixture = csharp_extension_method_receiver_variant_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "ReceiverController.Login",
        "--limit",
        "20",
    ]);
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "users.FormatForDisplay"
                    && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
            })
            .count()
            >= 2
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "maybeUsers.FormatForDisplay"
                    && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
            })
            .count()
            >= 2
    );
}

#[test]
fn cli_resolves_csharp_extension_method_collection_receivers() {
    let fixture = csharp_extension_method_collection_receiver_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "CollectionReceiverController.Login",
        "--limit",
        "20",
    ]);
    for expected_callee in [
        "servicePool.FormatForDisplay",
        "listUsers.FormatForDisplay",
        "usersById.FormatForDisplay",
    ] {
        assert!(callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == expected_callee
                && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
        }));
    }
}

#[test]
fn cli_resolves_csharp_extension_method_wrapper_receivers() {
    let fixture = csharp_extension_method_wrapper_receiver_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "WrapperReceiverController.Login",
        "--limit",
        "20",
    ]);
    for expected_callee in [
        "lazyUsers.Value.FormatForDisplay",
        "taskUsers.Result.FormatForDisplay",
        "valueTaskUsers.Result.FormatForDisplay",
    ] {
        assert!(callees.as_array().unwrap().iter().any(|call| {
            call["callee"] == expected_callee
                && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
        }));
    }
}

#[test]
fn cli_resolves_csharp_extension_method_temporary_receivers() {
    let fixture = csharp_extension_method_temporary_receiver_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "TemporaryReceiverController.Login",
        "--limit",
        "20",
    ]);
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "UserService.FormatForDisplay"
                    && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
            })
            .count()
            >= 4
    );
    assert!(
        callees
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| {
                call["callee"] == "App.Services.UserService.FormatForDisplay"
                    && call["callee_file"] == "src/App/Extensions/UserServiceExtensions.cs"
            })
            .count()
            >= 2
    );
}

#[test]
fn cli_resolves_rust_crate_and_super_use_imports() {
    let fixture = rust_use_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 11);
    assert_eq!(index["changed_files"], 11);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "30",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "crate::support::audit"
                    && dependency["resolved_file"] == "src/support/audit.rs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "super::support::helper"
                    && dependency["resolved_file"] == "src/controllers/support.rs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "self::nested::tool"
                    && dependency["resolved_file"] == "src/support/nested.rs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "plain"
                    && dependency["kind"] == "mod"
                    && dependency["resolved_file"] == "src/plain.rs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "Repository"
                    && dependency["kind"] == "base_type"
                    && dependency["local_alias"] == "Store"
                    && dependency["imported_symbol"] == "implements"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "plain" || dependency["resolved_file"] != "src/plain/mod.rs"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "serde::Serialize" && dependency["resolved_file"].is_null()
            })
    );

    let root_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "run",
        "--limit",
        "10",
    ]);
    assert!(root_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "audit.record" && call["callee_file"] == "src/support/audit.rs"
    }));

    let support_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "run_nested",
        "--limit",
        "10",
    ]);
    assert!(support_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "tool" && call["callee_file"] == "src/support/nested.rs"
    }));

    let auth_callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "login",
        "--limit",
        "10",
    ]);
    assert!(auth_callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "helper" && call["callee_file"] == "src/controllers/support.rs"
    }));
}

#[test]
fn cli_resolves_python_relative_imports() {
    let fixture = python_relative_import_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 7);
    assert_eq!(index["changed_files"], 7);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "30",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == ".support.audit"
                    && dependency["resolved_file"] == "app/controllers/support/audit.py"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == ".support"
                    && dependency["resolved_file"] == "app/controllers/support/__init__.py"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "..core.service"
                    && dependency["resolved_file"] == "app/core/service.py"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "app.shared"
                    && dependency["resolved_file"] == "app/shared/__init__.py"
                    && dependency["local_alias"] == "shared"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "app.shared.ping"
                    && dependency["resolved_file"] == "app/shared/__init__.py"
                    && dependency["local_alias"] == "shared_ping"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "app.shared.tools"
                    && dependency["resolved_file"] == "app/shared/tools.py"
                    && dependency["local_alias"] == "shared_tools"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "requests" && dependency["resolved_file"].is_null()
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "AuthController.login",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "audit.record"
            && call["callee_file"] == "app/controllers/support/audit.py"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "support.describe"
            && call["callee_file"] == "app/controllers/support/__init__.py"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "service.load" && call["callee_file"] == "app/core/service.py"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "shared.ping" && call["callee_file"] == "app/shared/__init__.py"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "shared_ping" && call["callee_file"] == "app/shared/__init__.py"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "shared_tools.pong" && call["callee_file"] == "app/shared/tools.py"
    }));
}

#[test]
fn cli_respects_null_package_imports_without_tsconfig_fallback() {
    let fixture = null_package_imports_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 8);
    assert_eq!(index["changed_files"], 8);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#enabled"
                    && dependency["resolved_file"] == "src/enabled-import.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#array"
                    && dependency["resolved_file"] == "src/array-import.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#conditional" && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "#conditional-external"
                    && dependency["resolved_file"].is_null()
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#conditional"
                    || dependency["resolved_file"] != "src/tsconfig-fallback.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#conditional-external"
                    || dependency["resolved_file"] != "src/default-external-import.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#conditional-external"
                    || dependency["resolved_file"] != "src/tsconfig-external-fallback.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#conditional-external"
                    || dependency["resolved_file"] != "external-import-lib.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "#array"
                    || dependency["resolved_file"] != "external-import-lib.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "nullImportMain",
        "--limit",
        "6",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "enabledImportRender" && call["callee_file"] == "src/enabled-import.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "arrayImportRender" && call["callee_file"] == "src/array-import.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalImportRender"
            || call["callee_file"] != "src/tsconfig-fallback.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalExternalImportRender"
            || call["callee_file"] != "src/default-external-import.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalExternalImportRender"
            || call["callee_file"] != "src/tsconfig-external-fallback.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "conditionalExternalImportRender"
            || call["callee_file"] != "external-import-lib.ts"
    }));
}

#[test]
fn cli_context_pack_routes_rust_trait_impl_relations() {
    let fixture = rust_use_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 11);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand repository trait implementation behavior",
        "--file",
        "src/repository.rs",
        "--token-budget",
        "4000",
    ]);

    let store_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/store.rs")
        .expect("rust impl should be selected through type relation evidence");
    assert!(
        store_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| {
                range["source"] == "type_relation"
                    && range["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("Type relation source of Repository"))
            })
    );
    let store_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/store.rs")
        .expect("rust impl should have a reading step");
    assert_eq!(store_step["next_action"], "inspect_type_relation");
}

#[test]
fn cli_overview_reports_type_relation_signals() {
    let fixture = rust_use_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 11);

    let overview = run_json(["overview", fixture.path().to_str().unwrap()]);
    assert!(
        overview["summary"]
            .as_str()
            .unwrap()
            .contains("type-relation edges")
    );
    assert!(
        overview["dependency_summary"]["type_relation_edges"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        overview["dependency_summary"]["top_type_relation_targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|target| target["target"] == "Repository" && target["edges"].as_u64() == Some(1))
    );
    assert!(
        overview["recommended_next_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| {
                tool["tool"] == "dependency_graph"
                    && tool["priority"].as_u64() == Some(25)
                    && tool["suggested_arguments"]["kinds"][0] == "base_type"
                    && tool["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("type-relation edges"))
            })
    );
    let priorities = overview["recommended_next_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["priority"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert!(
        priorities.windows(2).all(|window| window[0] <= window[1]),
        "recommended_next_tools must be sorted by priority: {priorities:?}"
    );
}

#[test]
fn cli_resolves_yarn_package_json_workspaces() {
    let fixture = yarn_workspace_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);
    assert_eq!(index["changed_files"], 6);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "yarn-ui/button"
                    && dependency["resolved_file"] == "packages/yarn-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "yarn-caret-ui/button"
                    && dependency["resolved_file"] == "packages/yarn-caret-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "yarn-tilde-ui/button"
                    && dependency["resolved_file"] == "packages/yarn-tilde-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "yarn-version-ui/button"
                    && dependency["resolved_file"] == "packages/yarn-version-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "yarn-legacy-ui/button"
                    && dependency["resolved_file"] == "node_modules/yarn-legacy-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "yarn-legacy-ui/button"
                    || dependency["resolved_file"] != "packages/legacy-yarn-ui/src/button.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "yarnWorkspaceMain",
        "--limit",
        "10",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "yarnButton" && call["callee_file"] == "packages/yarn-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "yarnCaretButton"
            && call["callee_file"] == "packages/yarn-caret-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "yarnTildeButton"
            && call["callee_file"] == "packages/yarn-tilde-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "yarnVersionButton"
            && call["callee_file"] == "packages/yarn-version-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "yarnLegacyButton"
            || call["callee_file"] != "packages/legacy-yarn-ui/src/button.ts"
    }));
}

#[test]
fn cli_resolves_package_json_workspace_array_exclusions() {
    let fixture = package_workspace_array_exclusion_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "array-ui/button"
                    && dependency["resolved_file"] == "packages/array-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "array-legacy-ui/button"
                    && dependency["resolved_file"] == "node_modules/array-legacy-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "array-legacy-ui/button"
                    || dependency["resolved_file"] != "packages/legacy-array-ui/src/button.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "arrayWorkspaceMain",
        "--limit",
        "6",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "arrayButton" && call["callee_file"] == "packages/array-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "arrayLegacyButton"
            || call["callee_file"] != "packages/legacy-array-ui/src/button.ts"
    }));
}

#[test]
fn cli_resolves_package_json_workspace_array_recursive_exclusions() {
    let fixture = package_workspace_array_recursive_exclusion_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "deep-array-ui/button"
                    && dependency["resolved_file"] == "packages/nested/deep-array-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "deep-legacy-array-ui/button"
                    && dependency["resolved_file"]
                        == "node_modules/deep-legacy-array-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "deep-legacy-array-ui/button"
                    || dependency["resolved_file"]
                        != "packages/legacy/deep-legacy-array-ui/src/button.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "recursiveArrayWorkspaceMain",
        "--limit",
        "6",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "deepArrayButton"
            && call["callee_file"] == "packages/nested/deep-array-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "deepLegacyArrayButton"
            || call["callee_file"] != "packages/legacy/deep-legacy-array-ui/src/button.ts"
    }));
}

#[test]
fn cli_resolves_yarn_workspace_object_recursive_exclusions() {
    let fixture = yarn_workspace_object_recursive_exclusion_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["changed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "deep-yarn-ui/button"
                    && dependency["resolved_file"] == "packages/nested/deep-yarn-ui/src/button.ts"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "deep-legacy-yarn-ui/button"
                    && dependency["resolved_file"]
                        == "node_modules/deep-legacy-yarn-ui/dist/button.js"
            })
    );
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .all(|dependency| {
                dependency["target"] != "deep-legacy-yarn-ui/button"
                    || dependency["resolved_file"]
                        != "packages/legacy/deep-legacy-yarn-ui/src/button.ts"
            })
    );

    let callees = run_json([
        "callees",
        fixture.path().to_str().unwrap(),
        "recursiveYarnWorkspaceMain",
        "--limit",
        "6",
    ]);
    assert!(callees.as_array().unwrap().iter().any(|call| {
        call["callee"] == "deepYarnButton"
            && call["callee_file"] == "packages/nested/deep-yarn-ui/src/button.ts"
    }));
    assert!(callees.as_array().unwrap().iter().all(|call| {
        call["callee"] != "deepLegacyYarnButton"
            || call["callee_file"] != "packages/legacy/deep-legacy-yarn-ui/src/button.ts"
    }));
}

#[test]
fn cli_uses_configured_package_conditions() {
    let fixture = package_conditions_fixture_project();

    let status = run_json(["config-status", fixture.path().to_str().unwrap()]);
    assert_eq!(status["loaded"], true);
    assert_eq!(status["configured_package_conditions"][0], "types");

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let deps = run_json([
        "dependency-graph",
        fixture.path().to_str().unwrap(),
        "--limit",
        "20",
    ]);
    assert!(
        deps["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| {
                dependency["target"] == "typed-lib"
                    && dependency["resolved_file"] == "node_modules/typed-lib/dist/index.d.ts"
            })
    );
}

#[test]
fn cli_reports_version_information() {
    Command::cargo_bin("codeinsight")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(is_match(r"^codeinsight \d+\.\d+\.\d+\n$").unwrap());

    let version = run_json(["version"]);

    assert_eq!(version["name"], "codeinsight");
    assert_eq!(version["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        version["target_arch"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        version["target_os"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[test]
fn cli_init_config_creates_sample_project_config() {
    let fixture = TempDir::new().unwrap();

    let created = run_json(["init-config", fixture.path().to_str().unwrap()]);
    let config_path = fixture.path().join(".codeinsight/config.toml");

    assert_eq!(created["created"], true);
    assert_eq!(created["overwritten"], false);
    assert_eq!(
        created["path"].as_str().unwrap(),
        config_path.canonicalize().unwrap().to_str().unwrap()
    );
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("[index]"));
    assert!(contents.contains("include = ["));
    assert!(contents.contains("exclude = ["));
    assert!(contents.contains("[javascript]"));
    assert!(contents.contains("package_conditions = ["));
    assert!(contents.contains("[impact_analysis]"));
    assert!(contents.contains("test_commands = []"));
    assert!(contents.contains("[[impact_analysis.suggested_checks]]"));

    Command::cargo_bin("codeinsight")
        .unwrap()
        .args(["init-config", fixture.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("already exists"));

    let overwritten = run_json(["init-config", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(overwritten["created"], false);
    assert_eq!(overwritten["overwritten"], true);
}

#[test]
fn cli_init_config_prefills_detected_test_commands() {
    let fixture = TempDir::new().unwrap();
    write_file(&fixture, "Cargo.toml", "[package]\nname = \"demo\"\n");
    write_file(&fixture, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    run_json(["init-config", fixture.path().to_str().unwrap()]);
    let contents =
        std::fs::read_to_string(fixture.path().join(".codeinsight/config.toml")).unwrap();

    assert!(contents.contains("test_commands = [\"cargo test --locked\", \"pnpm test\"]"));
}

#[test]
fn cli_agent_route_explains_when_all_backend_candidates_are_missing() {
    let fixture = fixture_project();
    let backend_evidence = serde_json::json!({
        "provider": "codebase-memory-mcp",
        "use_as_fallback": true,
        "prefer_for_context": true,
        "candidates": [
            { "file": "src/removed-auth.ts", "symbol": "AuthService" },
            { "file": "src/removed-main.ts", "symbol": "main" }
        ]
    })
    .to_string();

    let route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand invalid explicit seed",
        "--file",
        "does/not/exist.ts",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);

    let agreement = &route["routing_decision"]["backend_route_agreement"];
    assert_eq!(agreement["status"], "backend_unavailable");
    assert_eq!(
        agreement["recommended_action"],
        "provide_valid_backend_candidate"
    );
    assert_eq!(
        agreement["candidate_dispositions"],
        serde_json::json!([
            {
                "file": "src/removed-auth.ts",
                "rank": 1,
                "symbol": "AuthService",
                "context_status": "omitted",
                "context_reason": "missing_file",
                "next_action": "refresh_backend_evidence",
                "symbol_status": "not_checked"
            },
            {
                "file": "src/removed-main.ts",
                "rank": 2,
                "symbol": "main",
                "context_status": "omitted",
                "context_reason": "missing_file",
                "next_action": "refresh_backend_evidence",
                "symbol_status": "not_checked"
            }
        ])
    );
    assert_eq!(route["context_pack"]["files"], serde_json::json!([]));
    assert!(agreement.get("next_candidate_continuation").is_none());
    assert_eq!(
        route["routing_decision"]["continuation_source"],
        "context_pack"
    );
    assert_eq!(
        route["routing_decision"]["continuation_status"],
        route["context_pack"]["continuation_summary"]["status"]
    );
    assert_eq!(
        route["routing_decision"]["continuation_next_action"],
        route["context_pack"]["continuation_summary"]["next_action"]
    );
    assert_eq!(route["impact_status"], "skipped_invalid_seed");

    let local_route = run_json([
        "agent-route",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand app entrypoint flow",
        "--token-budget",
        "1600",
        "--force-index",
        "--backend-evidence-json",
        &backend_evidence,
    ]);
    let local_agreement = &local_route["routing_decision"]["backend_route_agreement"];
    assert_eq!(local_agreement["status"], "backend_unavailable");
    assert_eq!(
        local_agreement["recommended_action"],
        "read_selected_context"
    );
    assert_eq!(
        local_route["routing_decision"]["route_quality"]["recommended_action"],
        "read_selected_context"
    );
    assert!(
        local_route["routing_decision"]["route_quality"]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("none exist in the current local checkout"))
    );
}

#[test]
fn cli_config_status_reports_loaded_and_detected_commands() {
    let fixture = TempDir::new().unwrap();
    write_file(&fixture, "Cargo.toml", "[package]\nname = \"demo\"\n");

    let missing = run_json(["config-status", fixture.path().to_str().unwrap()]);
    assert_eq!(missing["exists"], false);
    assert_eq!(missing["loaded"], false);
    assert_eq!(missing["commands_override_builtin"], false);
    assert_eq!(
        missing["configured_package_conditions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(missing["detected_test_commands"][0], "cargo test --locked");

    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[javascript]
package_conditions = ["types", "import", "default"]

[index]
include = ["src/**"]
exclude = ["src/generated/**"]

[impact_analysis]
test_commands = ["cargo test -p core"]

[[impact_analysis.suggested_checks]]
command = "cargo test -p core integration"
languages = ["rust"]
"#,
    );

    let loaded = run_json(["config-status", fixture.path().to_str().unwrap()]);
    assert_eq!(loaded["exists"], true);
    assert_eq!(loaded["loaded"], true);
    assert_eq!(loaded["configured_test_commands"][0], "cargo test -p core");
    assert_eq!(loaded["configured_suggested_checks"].as_u64(), Some(1));
    assert_eq!(loaded["configured_package_conditions"][0], "types");
    assert_eq!(loaded["configured_index_includes"][0], "src/**");
    assert_eq!(loaded["configured_index_excludes"][0], "src/generated/**");
    assert_eq!(loaded["commands_override_builtin"], true);
    assert!(loaded.get("parse_error").is_none());
}

#[test]
fn cli_index_respects_configured_include_and_exclude_scope() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["src/**"]
exclude = ["src/generated/**"]
"#,
    );
    write_file(
        &fixture,
        "src/app.ts",
        r#"
export function keepScopedRoute() {
  return "keep";
}
"#,
    );
    write_file(
        &fixture,
        "src/generated/auto.ts",
        r#"
export function generatedScopedRoute() {
  return "skip";
}
"#,
    );
    write_file(
        &fixture,
        "docs/helper.ts",
        r#"
export function docsScopedRoute() {
  return "skip";
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["index_scope"]["enabled"].as_bool(), Some(true));
    assert_eq!(index["index_scope"]["includes"][0], "src/**");
    assert_eq!(index["index_scope"]["excludes"][0], "src/generated/**");
    assert_eq!(index["index_scope"]["walk_roots"][0], "src");
    assert_eq!(index["indexed_files"].as_u64(), Some(1));
    assert_eq!(index["changed_files"].as_u64(), Some(1));
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let kept = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "keepScopedRoute",
        "--limit",
        "5",
    ]);
    assert_eq!(kept.as_array().unwrap().len(), 1);

    let generated = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "generatedScopedRoute",
        "--limit",
        "5",
    ]);
    assert!(generated.as_array().unwrap().is_empty());

    let docs = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "docsScopedRoute",
        "--limit",
        "5",
    ]);
    assert!(docs.as_array().unwrap().is_empty());
}

#[test]
fn cli_index_reports_missing_configured_include_roots_without_full_walk() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["missing/**"]
"#,
    );
    write_file(
        &fixture,
        "src/app.ts",
        r#"
export function shouldNotBeIndexedFromMissingScope() {
  return "skip";
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"].as_u64(), Some(0));
    assert_eq!(index["index_scope"]["enabled"].as_bool(), Some(true));
    assert!(
        index["index_scope"]["walk_roots"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(index["errors"][0]["stage"], "scope");
    assert!(
        index["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("did not resolve")
    );
}

#[test]
fn cli_index_scope_replaces_existing_full_index_contents() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/app.ts",
        r#"
export function retainedScopedRoute() {
  return "keep";
}
"#,
    );
    write_file(
        &fixture,
        "docs/helper.ts",
        r#"
export function staleScopedRoute() {
  return "delete";
}
"#,
    );

    let full = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(full["indexed_files"].as_u64(), Some(2));

    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[index]
include = ["src"]
"#,
    );

    let scoped = run_json(["index", fixture.path().to_str().unwrap()]);
    assert_eq!(scoped["indexed_files"].as_u64(), Some(1));
    assert_eq!(scoped["unchanged_files"].as_u64(), Some(1));
    assert_eq!(scoped["deleted_files"].as_u64(), Some(1));

    let stale = run_json([
        "symbols",
        fixture.path().to_str().unwrap(),
        "staleScopedRoute",
        "--limit",
        "5",
    ]);
    assert!(stale.as_array().unwrap().is_empty());
}

#[test]
fn cli_config_status_reports_parse_errors_without_hiding_impact_failure() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "ok";
}
"#,
    );
    write_file(&fixture, "Cargo.toml", "[package]\nname = \"demo\"\n");

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    write_file(&fixture, ".codeinsight/config.toml", "[impact_analysis\n");

    let status = run_json(["config-status", fixture.path().to_str().unwrap()]);
    assert_eq!(status["exists"], true);
    assert_eq!(status["loaded"], false);
    assert_eq!(status["commands_override_builtin"], false);
    assert_eq!(
        status["configured_test_commands"].as_array().unwrap().len(),
        0
    );
    assert_eq!(status["configured_suggested_checks"].as_u64(), Some(0));
    assert_eq!(
        status["configured_package_conditions"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(status["detected_test_commands"][0], "cargo test --locked");
    assert!(
        status["parse_error"]
            .as_str()
            .is_some_and(|error| error.contains(".codeinsight/config.toml"))
    );

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "impact-analysis",
            fixture.path().to_str().unwrap(),
            "--symbol",
            "leaf",
        ])
        .assert()
        .failure()
        .stderr(contains("failed to parse"))
        .stderr(contains(".codeinsight/config.toml"));
}

#[test]
fn cli_indexes_checked_in_polyglot_fixture() {
    let fixture = copy_fixture("tests/fixtures/polyglot");

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 11);
    assert_eq!(index["changed_files"], 11);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    for (query, expected_language) in [
        ("WebController", "typescript"),
        ("legacyRender", "javascript"),
        ("AuthService", "python"),
        ("c_login", "c"),
        ("CppService", "cpp"),
        ("SharpService", "csharp"),
        ("StartServer", "go"),
        ("JavaService", "java"),
        ("PhpService", "php"),
        ("RenderService", "rust"),
        ("RubyService", "ruby"),
    ] {
        let symbols = run_json([
            "symbols",
            fixture.path().to_str().unwrap(),
            query,
            "--limit",
            "5",
        ]);
        assert!(
            symbols.as_array().unwrap().iter().any(|symbol| {
                symbol["name"] == query && symbol["language"] == expected_language
            }),
            "missing {query} symbol for {expected_language}"
        );
    }
}

#[test]
fn cli_semantic_index_explain_reports_chunk_changes() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/auth.py",
        r#"
class AuthService:
    def login(self, session):
        return session.get("cookie") == "fresh"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 1);

    let semantic_index = run_json([
        "semantic-index",
        fixture.path().to_str().unwrap(),
        "--chunk-lines",
        "20",
        "--explain",
    ]);
    let semantic_chunks = semantic_index["chunks"].as_u64().unwrap();
    assert_eq!(
        semantic_index["chunks_added"].as_u64(),
        Some(semantic_chunks)
    );
    let changes = semantic_index["changes"].as_array().unwrap();
    assert_eq!(changes.len() as u64, semantic_chunks);
    assert_eq!(changes[0]["change"], "added");
    assert_eq!(changes[0]["file"], "src/auth.py");
    assert_eq!(changes[0]["start_line"].as_u64(), Some(1));
    assert!(changes[0].get("previous_hash").is_none());
    assert!(changes[0]["content_hash"].as_str().unwrap().len() >= 32);

    let repeated_semantic_index = run_json([
        "semantic-index",
        fixture.path().to_str().unwrap(),
        "--chunk-lines",
        "20",
    ]);
    assert_eq!(repeated_semantic_index["chunks_added"].as_u64(), Some(0));
    assert!(repeated_semantic_index.get("changes").is_none());
}

#[test]
fn cli_overview_detects_framework_entrypoint_files() {
    let fixture = framework_entrypoint_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert!(index["indexed_files"].as_u64().unwrap() >= 4);

    let overview = run_json(["overview", fixture.path().to_str().unwrap()]);
    let entrypoints = overview["entrypoints"].as_array().unwrap();
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "app/page.tsx"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "Next.js app router entrypoint")
    );
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "pages/_app.tsx"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "Next.js pages bootstrap entrypoint")
    );
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "config/routes.rb"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "Rails route entrypoint")
    );
    assert!(entrypoints.iter().any(|entrypoint| entrypoint["file"]
        == "src/BillingApplication.java"
        && entrypoint["role"] == "source"
        && entrypoint["reason"] == "Java application entrypoint naming"));
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "manage.py"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "Python web framework entrypoint")
    );
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "project/urls.py"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "Python web framework entrypoint")
    );
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "src/Program.cs"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "C# web application entrypoint")
    );
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "src/Startup.cs"
                && entrypoint["role"] == "source"
                && entrypoint["reason"] == "C# web application entrypoint")
    );

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand launch sequence",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(context["seed_strategy"], "auto_entrypoint");
    assert_eq!(context["selected_seeds"][0]["value"], "app/page.tsx");
    assert_eq!(context["files"][0]["file"], "app/page.tsx");

    let route_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand routes",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(route_context["seed_strategy"], "auto_entrypoint");
    assert_eq!(
        route_context["selected_seeds"][0]["value"],
        "config/routes.rb"
    );
    assert_eq!(
        route_context["selected_seeds"][0]["source"],
        "overview_entrypoint"
    );
    assert!(
        route_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "routes")
    );
    assert_eq!(route_context["files"][0]["file"], "config/routes.rb");

    let url_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand django urls",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(url_context["seed_strategy"], "auto_entrypoint");
    assert_eq!(url_context["selected_seeds"][0]["value"], "project/urls.py");
    assert!(
        url_context["selected_seeds"][0]["matched_keywords"]
            .as_array()
            .unwrap()
            .iter()
            .any(|keyword| keyword == "urls")
    );
    assert_eq!(url_context["files"][0]["file"], "project/urls.py");
}

#[test]
fn cli_overview_prioritizes_source_entrypoints_over_script_helpers() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/main.ts",
        r#"
export function main() {
  return "application startup";
}

main();
"#,
    );
    write_file(
        &fixture,
        "scripts/adoption-comparison-smoke.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "supporting smoke helper"
}

main "$@"
"#,
    );
    write_file(
        &fixture,
        "scripts/adoption-comparison.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "supporting report helper"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);

    let overview = run_json(["overview", fixture.path().to_str().unwrap()]);
    let entrypoints = overview["entrypoints"].as_array().unwrap();
    assert_eq!(entrypoints[0]["file"], "src/main.ts");
    assert_eq!(entrypoints[0]["symbol"], "main");
    assert!(
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint["file"] == "scripts/adoption-comparison.sh"),
        "script entrypoints should remain visible after source entrypoints"
    );
}

#[test]
fn cli_context_pack_prefers_rust_entrypoint_over_script_helpers() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/main.rs",
        r#"
fn main() {
    run_server();
}

fn run_server() {}
"#,
    );
    write_file(
        &fixture,
        "scripts/framework-entrypoint-demo.sh",
        r#"
#!/usr/bin/env bash
set -euo pipefail

main() {
  echo "framework entrypoint helper"
}

main "$@"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand the CodeInsight MCP server Rust entrypoint",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(context["reading_plan"][0]["file"], "src/main.rs");
    assert_eq!(
        context["selected_seeds"][0]["source"],
        "overview_entrypoint"
    );
}

#[test]
fn cli_context_pack_prefers_task_named_package_source_over_demo_app_routes() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "packages/magazin/App.tsx",
        r#"
import { Route, Router } from "wouter";

export function App() {
  return (
    <Router>
      <Route path="/magazin">Magazine</Route>
    </Router>
  );
}
"#,
    );
    write_file(
        &fixture,
        "packages/magazin/index.tsx",
        r#"
import { App } from "./App";
import { mount } from "wouter";

mount(App);
"#,
    );
    write_file(
        &fixture,
        "packages/wouter/src/index.js",
        r#"
export function matchRoute(pattern, path) {
  return pattern === path;
}

export function route(pattern, component) {
  return { pattern, component, match: matchRoute };
}
"#,
    );
    write_file(
        &fixture,
        "packages/wouter/src/use-browser-location.js",
        r#"
export function useBrowserLocation() {
  const path = window.location.pathname;
  return [path, nextPath => window.history.pushState(null, "", nextPath)];
}
"#,
    );
    write_file(
        &fixture,
        "packages/wouter-preact/types/use-browser-location.d.ts",
        r#"
export declare function useBrowserLocation(): [
  path: string,
  navigate: (path: string) => void
];
"#,
    );
    write_file(
        &fixture,
        "packages/wouter/src/paths.js",
        r#"
export function relativePath(base, path) {
  return path.startsWith(base) ? path.slice(base.length) : path;
}

export function absolutePath(path, base) {
  return path.startsWith("~") ? path.slice(1) : base + path;
}
"#,
    );
    write_file(
        &fixture,
        "packages/wouter/src/memory-location.js",
        r#"
export function memoryLocation(path = "/") {
  let currentPath = path;
  return [() => currentPath, nextPath => { currentPath = nextPath; }];
}
"#,
    );
    write_file(
        &fixture,
        "packages/wouter/src/memory-location.d.ts",
        r#"
export declare function memoryLocation(path?: string): [
  getPath: () => string,
  navigate: (path: string) => void
];
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 8);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand wouter route matching flow",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(
        context["reading_plan"][0]["file"],
        "packages/wouter/src/index.js"
    );
    assert!(
        !context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["value"] == "packages/magazin/index.tsx")
    );

    let location_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand wouter browser location behavior",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(
        location_context["reading_plan"][0]["file"],
        "packages/wouter/src/use-browser-location.js"
    );

    let path_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand wouter path parser behavior",
        "--token-budget",
        "1200",
    ]);
    assert_eq!(
        path_context["reading_plan"][0]["file"],
        "packages/wouter/src/paths.js"
    );
}

#[test]
fn cli_impact_analysis_reports_depth_paths() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "ok";
}
"#,
    );
    write_file(
        &fixture,
        "src/service.ts",
        r#"
import { leaf } from "./core";

export function service() {
  return leaf();
}
"#,
    );
    write_file(
        &fixture,
        "src/route.ts",
        r#"
import { service } from "./service";

export function route() {
  return service();
}
"#,
    );
    write_file(
        &fixture,
        "src/core.test.ts",
        r#"
import { leaf } from "./core";

export function coreSpec() {
  return leaf();
}
"#,
    );
    write_file(&fixture, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "leaf",
        "--file",
        "src/core.ts",
        "--depth",
        "2",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "1",
    ]);
    assert_eq!(impact["depth"].as_u64(), Some(2));
    assert_eq!(impact["format"], "summary");
    assert_eq!(impact["evidence_limit"].as_u64(), Some(1));
    assert_eq!(impact["risk_level"], "high");
    assert!(
        impact["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("call-related files")
                && summary.contains("dependency-related files"))
    );
    assert!(
        impact["impact_counts"]["paths"].as_u64().unwrap()
            >= impact["paths"].as_array().unwrap().len() as u64
    );
    assert!(impact["impact_breakdown"]["seed_files"].as_u64().unwrap() >= 1);
    assert!(
        impact["impact_breakdown"]["symbol_definition_files"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        impact["impact_breakdown"]["call_related_files"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert!(
        impact["impact_breakdown"]["dependency_related_files"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert!(impact["impact_breakdown"]["call_paths"].as_u64().unwrap() >= 1);
    assert!(
        impact["impact_breakdown"]["dependency_paths"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(impact["symbols"].as_array().unwrap().len() <= 1);
    assert!(impact["references"].as_array().unwrap().len() <= 1);
    assert!(impact["callers"].as_array().unwrap().len() <= 1);
    assert!(impact["callees"].as_array().unwrap().len() <= 1);
    assert!(impact["dependencies"].as_array().unwrap().len() <= 1);
    assert!(
        impact["impact_counts"]["references"].as_u64().unwrap()
            > impact["references"].as_array().unwrap().len() as u64
    );
    assert!(
        impact["impact_counts"]["callers"].as_u64().unwrap()
            > impact["callers"].as_array().unwrap().len() as u64
    );
    let top_reasons = impact["top_reasons"].as_array().unwrap();
    assert!(top_reasons.iter().any(|reason| reason == "seed_file"));
    assert!(top_reasons.iter().any(|reason| {
        reason
            .as_str()
            .is_some_and(|value| value == "symbol_definition:leaf")
    }));
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "command" && check["command"] == "pnpm test" })
    );
    assert!(suggested_checks.iter().any(|check| {
        check["kind"] == "command"
            && check["command"] == "pnpm test -- src/core.test.ts"
            && check["reason"].as_str().is_some_and(|reason| {
                reason.contains("Focused test file src/core.test.ts is impacted")
            })
    }));
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "review" && check["file"] == "src/core.ts" })
    );
    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/route.ts")
    );
    assert!(
        impact["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| { path["kind"] == "call" && path["depth"] == 2 && path["to"] == "route" })
    );
    assert!(impact["paths"].as_array().unwrap().iter().any(|path| {
        path["kind"] == "dependency" && path["depth"] == 2 && path["to"] == "src/route.ts"
    }));

    let downstream_impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "service",
        "--depth",
        "2",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "1",
    ]);
    assert!(
        downstream_impact["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| {
                path["kind"] == "call"
                    && path["depth"] == 1
                    && path["from"] == "service"
                    && path["to"] == "leaf"
                    && path["file"] == "src/core.ts"
            })
    );
}

#[test]
fn cli_impact_analysis_reports_type_relation_paths() {
    let fixture = java_source_import_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 6);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--file",
        "src/main/java/com/example/app/BaseApp.java",
        "--depth",
        "2",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "3",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |file| file["file"] == "src/main/java/com/example/app/App.java"
                    && file["reasons"].as_array().unwrap().iter().any(|reason| {
                        reason
                            .as_str()
                            .is_some_and(|reason| reason.starts_with("type_relation_source:App"))
                    })
            )
    );
    assert!(impact["paths"].as_array().unwrap().iter().any(|path| {
        path["kind"] == "type_relation"
            && path["from"] == "src/main/java/com/example/app/BaseApp.java"
            && path["to"] == "src/main/java/com/example/app/App.java"
            && path["via"] == "extends:BaseApp"
    }));
    assert!(
        impact["impact_breakdown"]["dependency_related_files"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        impact["impact_breakdown"]["dependency_paths"]
            .as_u64()
            .unwrap()
            >= 1
    );
}

#[test]
fn cli_impact_analysis_prefers_configured_suggested_checks() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "ok";
}
"#,
    );
    write_file(
        &fixture,
        "src/route.ts",
        r#"
import { leaf } from "./core";

export function route() {
  return leaf();
}
"#,
    );
    write_file(&fixture, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
[impact_analysis]
test_commands = ["pnpm exec vitest run --changed"]

[[impact_analysis.suggested_checks]]
command = "pnpm exec vitest run src/core.test.ts"
reason = "Configured focused TypeScript test."
languages = ["typescript"]
files = ["src/core"]
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "leaf",
        "--file",
        "src/core.ts",
        "--depth",
        "2",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "1",
    ]);
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(suggested_checks.iter().any(|check| {
        check["kind"] == "command" && check["command"] == "pnpm exec vitest run --changed"
    }));
    assert!(suggested_checks.iter().any(|check| {
        check["kind"] == "command"
            && check["command"] == "pnpm exec vitest run src/core.test.ts"
            && check["reason"] == "Configured focused TypeScript test."
    }));
    assert!(
        !suggested_checks
            .iter()
            .any(|check| check["command"] == "pnpm test")
    );
}

#[test]
fn cli_impact_analysis_suggests_focused_rust_integration_tests() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "Cargo.toml",
        r#"
[package]
name = "demo"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(
        &fixture,
        "src/lib.rs",
        r#"
pub fn normalize_token(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}
"#,
    );
    write_file(
        &fixture,
        "tests/normalize.rs",
        r#"
use demo::normalize_token;

#[test]
fn covers_normalize_token() {
    assert_eq!(normalize_token(" Demo "), "demo");
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "normalize_token",
        "--file",
        "src/lib.rs",
        "--depth",
        "1",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "2",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "tests/normalize.rs")
    );
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(
        suggested_checks.iter().any(|check| {
            check["kind"] == "command" && check["command"] == "cargo test --locked"
        }),
        "expected full Cargo test command in {suggested_checks:#?}"
    );
    assert!(
        suggested_checks.iter().any(|check| {
            check["kind"] == "command"
                && check["command"] == "cargo test --locked --test normalize"
                && check["reason"].as_str().is_some_and(|reason| {
                    reason.contains("Focused test file tests/normalize.rs is impacted")
                })
        }),
        "expected focused Cargo integration test command in {suggested_checks:#?}"
    );
}

#[test]
fn cli_impact_analysis_suggests_focused_ruby_specs() {
    let fixture = TempDir::new().unwrap();
    write_file(&fixture, "Gemfile", "source 'https://rubygems.org'\n");
    write_file(
        &fixture,
        "src/core.rb",
        r#"
module DemoCore
  def self.normalize_token(input)
    input.strip.downcase
  end
end
"#,
    );
    write_file(
        &fixture,
        "spec/core_spec.rb",
        r#"
require_relative "../src/core"

RSpec.describe DemoCore do
  it "normalizes tokens" do
    expect(DemoCore.normalize_token(" Demo ")).to eq("demo")
  end
end
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "normalize_token",
        "--file",
        "src/core.rb",
        "--depth",
        "1",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "2",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "spec/core_spec.rb")
    );
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "command" && check["command"] == "bundle exec rspec" }),
        "expected full RSpec command in {suggested_checks:#?}"
    );
    assert!(
        suggested_checks.iter().any(|check| {
            check["kind"] == "command"
                && check["command"] == "bundle exec rspec spec/core_spec.rb"
                && check["reason"].as_str().is_some_and(|reason| {
                    reason.contains("Focused test file spec/core_spec.rb is impacted")
                })
        }),
        "expected focused RSpec command in {suggested_checks:#?}"
    );
}

#[test]
fn cli_impact_analysis_suggests_focused_java_tests() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "pom.xml",
        r#"
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
  <version>1.0.0</version>
</project>
"#,
    );
    write_file(
        &fixture,
        "src/main/java/com/example/TokenNormalizer.java",
        r#"
package com.example;

public class TokenNormalizer {
    public static String normalizeToken(String input) {
        return input.trim().toLowerCase();
    }
}
"#,
    );
    write_file(
        &fixture,
        "src/test/java/com/example/TokenNormalizerTest.java",
        r#"
package com.example;

public class TokenNormalizerTest {
    public void coversNormalizeToken() {
        TokenNormalizer.normalizeToken(" Demo ");
    }
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "normalizeToken",
        "--file",
        "src/main/java/com/example/TokenNormalizer.java",
        "--depth",
        "1",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "2",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "src/test/java/com/example/TokenNormalizerTest.java")
    );
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "command" && check["command"] == "mvn test" }),
        "expected full Maven test command in {suggested_checks:#?}"
    );
    assert!(
        suggested_checks.iter().any(|check| {
            check["kind"] == "command"
                && check["command"] == "mvn -Dtest=TokenNormalizerTest test"
                && check["reason"].as_str().is_some_and(|reason| {
                    reason.contains(
                        "Focused test file src/test/java/com/example/TokenNormalizerTest.java is impacted",
                    )
                })
        }),
        "expected focused Maven test command in {suggested_checks:#?}"
    );
}

#[test]
fn cli_impact_analysis_suggests_focused_csharp_tests() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "Demo.csproj",
        r#"
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
</Project>
"#,
    );
    write_file(
        &fixture,
        "src/TokenNormalizer.cs",
        r#"
namespace Demo;

public static class TokenNormalizer {
    public static string NormalizeToken(string input) {
        return input.Trim().ToLowerInvariant();
    }
}
"#,
    );
    write_file(
        &fixture,
        "tests/TokenNormalizerTests.cs",
        r#"
namespace Demo.Tests;

public class TokenNormalizerTests {
    public void CoversNormalizeToken() {
        TokenNormalizer.NormalizeToken(" Demo ");
    }
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "NormalizeToken",
        "--file",
        "src/TokenNormalizer.cs",
        "--depth",
        "1",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "2",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "tests/TokenNormalizerTests.cs")
    );
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "command" && check["command"] == "dotnet test" }),
        "expected full dotnet test command in {suggested_checks:#?}"
    );
    assert!(
        suggested_checks.iter().any(|check| {
            check["kind"] == "command"
                && check["command"]
                    == "dotnet test --filter FullyQualifiedName~TokenNormalizerTests"
                && check["reason"].as_str().is_some_and(|reason| {
                    reason.contains("Focused test file tests/TokenNormalizerTests.cs is impacted")
                })
        }),
        "expected focused dotnet test command in {suggested_checks:#?}"
    );
}

#[test]
fn cli_impact_analysis_keeps_php_composer_checks_broad() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "composer.json",
        r#"
{
  "scripts": {
    "test": "phpunit"
  }
}
"#,
    );
    write_file(
        &fixture,
        "src/TokenNormalizer.php",
        r#"
<?php

namespace Demo;

class TokenNormalizer
{
    public static function normalizeToken(string $input): string
    {
        return strtolower(trim($input));
    }
}
"#,
    );
    write_file(
        &fixture,
        "tests/TokenNormalizerTest.php",
        r#"
<?php

namespace Demo\Tests;

use Demo\TokenNormalizer;

class TokenNormalizerTest
{
    public function testNormalizeToken(): void
    {
        TokenNormalizer::normalizeToken(" Demo ");
    }
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let impact = run_json([
        "impact-analysis",
        fixture.path().to_str().unwrap(),
        "--symbol",
        "normalizeToken",
        "--file",
        "src/TokenNormalizer.php",
        "--depth",
        "1",
        "--limit",
        "20",
        "--format",
        "summary",
        "--evidence-limit",
        "2",
    ]);

    assert!(
        impact["impacted_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "tests/TokenNormalizerTest.php")
    );
    let suggested_checks = impact["suggested_checks"].as_array().unwrap();
    assert_eq!(suggested_checks[0]["command"], "composer test");
    assert!(
        suggested_checks
            .iter()
            .any(|check| { check["kind"] == "command" && check["command"] == "composer test" }),
        "expected broad Composer test command in {suggested_checks:#?}"
    );
    assert!(
        !suggested_checks.iter().any(|check| {
            check["command"].as_str().is_some_and(|command| {
                command.contains("TokenNormalizerTest") || command.contains("tests/")
            })
        }),
        "Composer scripts do not have portable file forwarding; expected no built-in focused PHP command in {suggested_checks:#?}"
    );
}

#[test]
fn cli_find_references_filters_comments_and_downranks_tests() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "ok";
}
"#,
    );
    write_file(
        &fixture,
        "src/route.ts",
        r#"
import { leaf } from "./core";

// leaf should not be returned from comments.
const note = "leaf should not be returned from strings";

export function route() {
  return leaf();
}
"#,
    );
    write_file(
        &fixture,
        "src/core.test.ts",
        r#"
import { leaf } from "./core";

export function testLeaf() {
  return leaf();
}
"#,
    );
    write_file(
        &fixture,
        "src/native.c",
        r#"
#define LEAF_MACRO 1

int use_macro(void) {
  return LEAF_MACRO;
}
"#,
    );
    write_file(
        &fixture,
        "src/lib.rs",
        r#"
pub(crate) async fn load_user() -> String {
    "ok".to_string()
}

pub async fn run() -> String {
    load_user().await
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 5);

    let references = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "leaf",
        "--limit",
        "10",
    ]);
    let references = references.as_array().unwrap();
    assert!(
        references
            .iter()
            .any(|reference| reference["file"] == "src/route.ts"
                && reference["reference_kind"] == "call"
                && reference["context"].as_str().unwrap().contains("leaf()"))
    );
    assert!(
        references
            .iter()
            .any(|reference| reference["file"] == "src/core.test.ts")
    );
    assert!(
        !references.iter().any(|reference| reference["context"]
            .as_str()
            .is_some_and(|context| context.contains("comments") || context.contains("strings"))),
        "comment and string-only references should be filtered"
    );
    assert!(
        !references.iter().any(|reference| reference["context"]
            .as_str()
            .is_some_and(|context| context.contains("export function leaf"))),
        "definitions should be filtered unless explicitly requested"
    );

    let production_call_index = references
        .iter()
        .position(|reference| {
            reference["file"] == "src/route.ts"
                && reference["reference_kind"] == "call"
                && reference["context"].as_str().unwrap().contains("leaf()")
        })
        .unwrap();
    let test_call_index = references
        .iter()
        .position(|reference| {
            reference["file"] == "src/core.test.ts"
                && reference["reference_kind"] == "call"
                && reference["context"].as_str().unwrap().contains("leaf()")
        })
        .unwrap();
    assert!(
        production_call_index < test_call_index,
        "production references should rank before test references"
    );
    assert!(
        references[test_call_index]["confidence"].as_f64().unwrap()
            < references[production_call_index]["confidence"]
                .as_f64()
                .unwrap()
    );

    let macro_references = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "LEAF_MACRO",
        "--include-definitions",
        "--limit",
        "10",
    ]);
    assert!(
        macro_references
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| {
                reference["file"] == "src/native.c"
                    && reference["reference_kind"] == "definition"
                    && reference["context"].as_str().unwrap().contains("#define")
            })
    );

    let leaf_with_definitions = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "leaf",
        "--include-definitions",
        "--limit",
        "10",
    ]);
    assert!(
        leaf_with_definitions
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| {
                reference["file"] == "src/core.ts"
                    && reference["reference_kind"] == "definition"
                    && reference["context"]
                        .as_str()
                        .unwrap()
                        .contains("export function leaf")
            })
    );

    let rust_references = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "load_user",
        "--limit",
        "10",
    ]);
    let rust_references = rust_references.as_array().unwrap();
    assert!(
        rust_references.iter().any(|reference| {
            reference["file"] == "src/lib.rs"
                && reference["reference_kind"] == "call"
                && reference["context"]
                    .as_str()
                    .unwrap()
                    .contains("load_user().await")
        }),
        "expected Rust call reference in {rust_references:#?}"
    );
    assert!(
        !rust_references.iter().any(|reference| reference["context"]
            .as_str()
            .is_some_and(|context| context.contains("pub(crate) async fn load_user"))),
        "Rust pub async definitions should be filtered unless explicitly requested"
    );

    let rust_with_definitions = run_json([
        "find-references",
        fixture.path().to_str().unwrap(),
        "load_user",
        "--include-definitions",
        "--limit",
        "10",
    ]);
    assert!(
        rust_with_definitions
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| {
                reference["file"] == "src/lib.rs"
                    && reference["reference_kind"] == "definition"
                    && reference["context"]
                        .as_str()
                        .unwrap()
                        .contains("pub(crate) async fn load_user")
            })
    );
}

#[test]
fn cli_context_pack_downranks_test_references() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "src/core.ts",
        r#"
export function leaf() {
  return "ok";
}
"#,
    );
    write_file(
        &fixture,
        "src/route.ts",
        r#"
import { leaf } from "./core";

export function route() {
  return leaf();
}
"#,
    );
    write_file(
        &fixture,
        "src/core.test.ts",
        r#"
import { leaf } from "./core";

export function spec() {
  return leaf();
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand leaf production behavior",
        "--symbol",
        "leaf",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(context["seed_strategy"], "explicit");
    let context_files = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    let route_index = context_files
        .iter()
        .position(|file| *file == "src/route.ts")
        .unwrap();
    let test_index = context_files
        .iter()
        .position(|file| *file == "src/core.test.ts")
        .unwrap();
    assert!(
        route_index < test_index,
        "production references should rank before test references in context_pack"
    );
    let route_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/route.ts")
        .unwrap();
    assert_eq!(route_file["source"], "call_graph");
    assert!(route_file["score"].as_i64().unwrap() > 0);
    assert!(
        route_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| range["source"] == "call_graph" && range["score"].as_i64().unwrap() > 0)
    );
    assert!(
        route_file["reason"]
            .as_str()
            .unwrap()
            .contains("via call_graph")
    );
    assert!(
        route_file["reason"]
            .as_str()
            .unwrap()
            .contains("evidence mix")
    );
    assert_eq!(route_file["source_mix"][0]["source"], "call graph");
    assert_eq!(route_file["source_mix"][0]["count"].as_u64(), Some(1));

    let fallback_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand production behavior",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(fallback_context["seed_strategy"], "auto_source_fallback");
    assert!(
        fallback_context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|seed| seed["role"] == "source")
    );
    assert!(
        fallback_context["selected_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|seed| seed["value"] == "src/core.ts")
    );

    let test_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand leaf test coverage",
        "--symbol",
        "leaf",
        "--token-budget",
        "1600",
    ]);
    let test_context_files = test_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect::<Vec<_>>();
    let route_index = test_context_files
        .iter()
        .position(|file| *file == "src/route.ts")
        .unwrap();
    let test_index = test_context_files
        .iter()
        .position(|file| *file == "src/core.test.ts")
        .unwrap();
    assert!(
        test_index < route_index,
        "test-related tasks should rank test references before production callers"
    );
    let test_file = test_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/core.test.ts")
        .unwrap();
    assert_eq!(test_file["source"], "call_graph");
    let test_route_file = test_context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/route.ts")
        .unwrap();
    assert!(test_file["score"].as_i64().unwrap() > test_route_file["score"].as_i64().unwrap());
    assert!(
        test_file["reason"]
            .as_str()
            .unwrap()
            .contains("via call_graph")
    );

    let seed_test_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand leaf behavior",
        "--file",
        "src/core.test.ts",
        "--symbol",
        "leaf",
        "--token-budget",
        "1600",
    ]);
    assert_eq!(seed_test_context["files"][0]["file"], "src/core.test.ts");
    assert_eq!(seed_test_context["files"][0]["source"], "seed_file");
    assert!(seed_test_context["files"][0]["score"].as_i64().unwrap() > 0);
    assert_eq!(
        seed_test_context["files"][0]["ranges"][0]["source"],
        "seed_file"
    );
    assert!(
        seed_test_context["files"][0]["ranges"][0]["score"]
            .as_i64()
            .unwrap()
            > 0
    );
    assert!(
        seed_test_context["files"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("via seed_file")
    );
    assert!(
        seed_test_context["files"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("evidence mix")
    );
    assert_eq!(
        seed_test_context["files"][0]["source_mix"][0]["source"],
        "seed file"
    );
}

#[test]
fn cli_context_pack_scopes_dependency_suggested_tool() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "app/main.py",
        r#"
from . import support

class Entry:
    pass
"#,
    );
    write_file(
        &fixture,
        "app/support.py",
        r#"
def helper():
    return "ok"
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand authentication support dependency",
        "--file",
        "app/main.py",
        "--token-budget",
        "1800",
    ]);
    let dependency_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "app/support.py")
        .unwrap();
    assert_eq!(dependency_step["next_action"], "inspect_dependency");
    assert_eq!(
        dependency_step["suggested_tool"]["tool"],
        "dependency_graph"
    );
    assert_eq!(
        dependency_step["suggested_tool"]["suggested_arguments"]["files"][0],
        "app/support.py"
    );
    assert_eq!(
        dependency_step["suggested_tool"]["suggested_arguments"]["limit"].as_u64(),
        Some(100)
    );
    let dependency_question = dependency_step["question"].as_str().unwrap();
    assert!(dependency_question.contains("authentication"));
    assert!(dependency_question.contains("session boundaries"));
    let dependency_focus = dependency_step["focus"].as_str().unwrap();
    assert!(dependency_focus.contains("authentication"));
    assert!(dependency_focus.contains("session"));
}

#[test]
fn cli_context_pack_routes_csharp_base_type_relations() {
    let fixture = csharp_using_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 15);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand inherited authentication controller behavior",
        "--file",
        "src/App/Controllers/AuthController.cs",
        "--token-budget",
        "6000",
    ]);

    let base_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/App/Controllers/BaseController.cs")
        .expect("base controller should be selected through type relation evidence");
    assert!(
        base_file["source_mix"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source"] == "type relation")
    );
    assert!(base_file["ranges"].as_array().unwrap().iter().any(|range| {
        range["source"] == "type_relation"
            && range["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("base type App.Controllers.BaseController"))
    }));

    let base_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "src/App/Controllers/BaseController.cs")
        .expect("base controller should have a reading step");
    assert_eq!(base_step["next_action"], "inspect_type_relation");
    assert_eq!(base_step["suggested_tool"]["tool"], "dependency_graph");
    assert!(
        base_step["question"]
            .as_str()
            .unwrap()
            .contains("base types")
    );
    assert!(
        base_step["focus"]
            .as_str()
            .unwrap()
            .contains("authentication")
    );

    let interface_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "src/App/Contracts/IAuthController.cs")
        .expect("implemented interface should be selected through type relation evidence");
    assert!(
        interface_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| {
                range["source"] == "type_relation"
                    && range["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("base type IAuthController"))
            })
    );
}

#[test]
fn cli_context_pack_uses_imported_callee_file_hints() {
    let fixture = ruby_require_relative_fixture_project();

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand login audit behavior",
        "--symbol",
        "Example.AuthService.login",
        "--token-budget",
        "1800",
    ]);
    assert_eq!(context["seed_strategy"], "explicit");

    let audit_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "lib/support/audit.rb")
        .unwrap();
    assert_eq!(audit_file["source"], "call_graph");
    assert!(audit_file["score"].as_i64().unwrap() > 0);
    assert!(
        audit_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| {
                range["source"] == "call_graph"
                    && range["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("Audit.record"))
            })
    );
    assert!(
        audit_file["reason"]
            .as_str()
            .unwrap()
            .contains("evidence mix")
    );
    assert_eq!(audit_file["source_mix"][0]["source"], "call graph");
    let audit_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "lib/support/audit.rb")
        .unwrap();
    assert_eq!(audit_step["next_action"], "follow_call_graph");
    assert_eq!(audit_step["suggested_tool"]["tool"], "impact_analysis");
    assert_eq!(
        audit_step["suggested_tool"]["suggested_arguments"]["files"][0],
        "lib/support/audit.rb"
    );
    assert_eq!(
        audit_step["suggested_tool"]["suggested_arguments"]["evidence_limit"].as_u64(),
        Some(5)
    );
    let audit_question = audit_step["question"].as_str().unwrap();
    assert!(audit_question.contains("authentication decisions"));
    assert!(audit_question.contains("session state"));

    let impact_context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "assess call impact paths for audit behavior",
        "--symbol",
        "Example.AuthService.login",
        "--token-budget",
        "1800",
    ]);
    let impact_audit_step = impact_context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "lib/support/audit.rb")
        .unwrap();
    assert_eq!(impact_audit_step["next_action"], "follow_call_graph");
    assert_eq!(
        impact_audit_step["suggested_tool"]["tool"],
        "impact_analysis"
    );
    assert_eq!(
        impact_audit_step["suggested_tool"]["suggested_arguments"]["files"][0],
        "lib/support/audit.rb"
    );
    let impact_audit_focus = impact_audit_step["focus"].as_str().unwrap();
    assert!(impact_audit_focus.contains("callers"));
    assert!(impact_audit_focus.contains("callees"));
    assert!(impact_audit_focus.contains("impact paths"));
    let impact_audit_question = impact_audit_step["question"].as_str().unwrap();
    assert!(impact_audit_question.contains("callers"));
    assert!(impact_audit_question.contains("callees"));
    assert!(impact_audit_question.contains("impact paths"));
}

#[test]
fn cli_context_pack_uses_bash_source_callee_file_hints() {
    let fixture = TempDir::new().unwrap();
    write_file(
        &fixture,
        "scripts/bootstrap.sh",
        r#"
#!/usr/bin/env bash
source ./lib/common.sh

bootstrap() {
  run_common
}
"#,
    );
    write_file(
        &fixture,
        "scripts/lib/common.sh",
        r#"
#!/usr/bin/env bash

run_common() {
  echo "ready"
}
"#,
    );

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 2);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let context = run_json([
        "context-pack",
        fixture.path().to_str().unwrap(),
        "--task",
        "understand shell bootstrap helper behavior",
        "--symbol",
        "bootstrap",
        "--token-budget",
        "1600",
    ]);

    assert_eq!(context["seed_strategy"], "explicit");
    assert_eq!(context["files"][0]["file"], "scripts/bootstrap.sh");
    let helper_file = context["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["file"] == "scripts/lib/common.sh")
        .expect("sourced helper should be selected through call graph hints");
    assert!(
        helper_file["source_mix"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source"] == "call graph")
    );
    assert!(
        helper_file["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|range| {
                range["source"] == "call_graph"
                    && range["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("run_common"))
            })
    );
    let helper_step = context["reading_plan"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["file"] == "scripts/lib/common.sh")
        .expect("sourced helper should have a reading step");
    assert_eq!(helper_step["next_action"], "follow_call_graph");
    assert_eq!(helper_step["suggested_tool"]["tool"], "impact_analysis");
}

#[test]
fn cli_semantic_search_requires_embedding_provider() {
    let fixture = fixture_project();

    Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args([
            "semantic-search",
            fixture.path().to_str().unwrap(),
            "authentication flow",
        ])
        .assert()
        .failure()
        .stderr(contains("CODEINSIGHT_EMBEDDING_PROVIDER=local-hash"));
}

#[test]
fn cli_embedding_status_reports_ollama_config_without_network_call() {
    let status = run_json_with_env(
        ["embedding-status"],
        [
            ("CODEINSIGHT_EMBEDDING_PROVIDER", "ollama"),
            ("CODEINSIGHT_OLLAMA_BASE_URL", "http://127.0.0.1:9999"),
            ("CODEINSIGHT_OLLAMA_EMBEDDING_MODEL", "nomic-embed-text"),
            ("CODEINSIGHT_OLLAMA_TIMEOUT_SECS", "7"),
            ("CODEINSIGHT_EMBEDDING_BATCH_SIZE", "3"),
        ],
    );

    assert_eq!(status["provider"], "ollama");
    assert_eq!(status["model"], "nomic-embed-text");
    assert_eq!(status["configured"], true);
    assert_eq!(status["ollama"]["base_url"], "http://127.0.0.1:9999");
    assert_eq!(status["ollama"]["timeout_secs"].as_u64(), Some(7));
    assert_eq!(status["batch_size"].as_u64(), Some(3));
    assert_eq!(status["batch_size_env"], "CODEINSIGHT_EMBEDDING_BATCH_SIZE");
}

#[test]
fn cli_embedding_status_reports_openai_config_without_exposing_key() {
    let status = run_json_with_env(
        ["embedding-status"],
        [
            ("CODEINSIGHT_EMBEDDING_PROVIDER", "openai"),
            ("CODEINSIGHT_OPENAI_API_KEY", "sk-test-secret"),
            ("CODEINSIGHT_OPENAI_BASE_URL", "https://example.test/v1/"),
            (
                "CODEINSIGHT_OPENAI_EMBEDDING_MODEL",
                "text-embedding-3-large",
            ),
            ("CODEINSIGHT_OPENAI_TIMEOUT_SECS", "11"),
        ],
    );

    assert_eq!(status["provider"], "openai");
    assert_eq!(status["model"], "text-embedding-3-large");
    assert_eq!(status["configured"], true);
    assert_eq!(status["openai"]["base_url"], "https://example.test/v1");
    assert_eq!(
        status["openai"]["api_key_env"],
        "CODEINSIGHT_OPENAI_API_KEY"
    );
    assert_eq!(status["openai"]["api_key_configured"], true);
    assert_eq!(status["openai"]["timeout_secs"].as_u64(), Some(11));
    assert!(
        !serde_json::to_string(&status)
            .unwrap()
            .contains("sk-test-secret")
    );
}

#[test]
fn mcp_stdio_executes_symbol_search() {
    let fixture = fixture_project();
    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "symbol_search",
            "arguments": {
                "root": fixture.path(),
                "query": "AuthService",
                "limit": 3
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["structuredContent"][0]["name"],
        "AuthService"
    );
}

#[test]
fn mcp_stdio_executes_agent_route() {
    let fixture = fixture_project();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand app entrypoint flow",
                "token_budget": 1600,
                "force_index": true,
                "impact_limit": 10,
                "impact_depth": 2,
                "impact_evidence_limit": 3,
                "backend_evidence": {
                    "provider": "codebase-memory-mcp",
                    "candidate_files": ["src/main.ts"],
                    "evidence_sources": ["entry_points"],
                    "evidence_count": 4,
                    "latency_ms": 19,
                    "confidence": 0.88
                }
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 3);
    assert_eq!(route["context_pack"]["seed_strategy"], "auto_entrypoint");
    assert_eq!(route["impact_status"], "complete");
    assert_eq!(route["impact_analysis"]["depth"].as_u64(), Some(2));
    assert_eq!(
        route["route"][0]["tool"],
        serde_json::Value::String("index_project".to_string())
    );
    assert_eq!(
        route["route"][3]["tool"],
        serde_json::Value::String("impact_analysis".to_string())
    );
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["provider"],
        "codebase-memory-mcp"
    );
    assert!(
        route["routing_decision"]["route_quality"]["confidence_factors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|factor| factor.as_str().unwrap().contains(
                "backend codebase-memory-mcp independently selected the same first file"
            ))
    );
    let context_reason = route["route"][2]["reason"].as_str().unwrap();
    assert!(context_reason.contains("read src/main.ts first"));
    assert!(context_reason.contains("candidate rank 1"));
    assert!(context_reason.contains("file_outline"));
    assert_eq!(
        route["execution_plan"][0]["action"],
        "read_selected_context"
    );
    assert_eq!(
        route["execution_plan"][1]["action"],
        "use_current_reading_step_suggested_tool"
    );
    assert_eq!(
        route["execution_plan"][1]["suggested_tool"]["tool"],
        "file_outline"
    );
    assert_eq!(
        route["execution_plan"][2]["action"],
        "use_continuation_if_needed"
    );
    assert_eq!(
        route["execution_plan"][3]["action"],
        "review_impact_before_edits"
    );
    assert_agent_route_execution_plan_matches_context(route);
    let impact_reason = route["route"][3]["reason"].as_str().unwrap();
    assert!(impact_reason.contains("pre-edit impact check"));
    assert!(impact_reason.contains("call-related files"));
    assert!(impact_reason.contains("dependency-related files"));
}

#[test]
fn mcp_stdio_agent_route_returns_blocked_plan_for_empty_repository() {
    let fixture = TempDir::new().unwrap();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 31,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand this repository",
                "token_budget": 1000,
                "force_index": true
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 31);
    assert!(response["error"].is_null());
    assert_eq!(route["route"][2]["status"], "blocked_no_seed");
    assert_eq!(route["impact_status"], "skipped_no_seed");
    assert_eq!(route["context_pack"]["seed_strategy"], "auto_no_seed");
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_seed_file_or_symbol"
    );
    assert!(route["current_reading_step"].is_null());
    assert_eq!(
        route["execution_plan"][0]["status"],
        "blocked_no_reading_plan"
    );
    assert_eq!(
        route["execution_plan"][1]["status"],
        "blocked_no_current_reading_step"
    );
    assert_eq!(
        route["execution_plan"][3]["action"],
        "review_impact_before_edits"
    );
}

#[test]
fn mcp_stdio_agent_route_returns_blocked_plan_for_invalid_seed_file() {
    let fixture = fixture_project();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 33,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand invalid explicit seed",
                "files": ["does/not/exist.ts"],
                "token_budget": 1000,
                "force_index": true
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 33);
    assert!(response["error"].is_null());
    assert_eq!(route["route"][2]["status"], "blocked_invalid_seed");
    assert_eq!(route["impact_status"], "skipped_invalid_seed");
    assert_eq!(
        route["context_pack"]["seed_strategy"],
        "explicit_invalid_seed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_existing_seed_file_or_symbol"
    );
    assert!(route["current_reading_step"].is_null());
    assert_eq!(
        route["execution_plan"][0]["status"],
        "blocked_no_reading_plan"
    );
    assert_eq!(route["execution_plan"][3]["status"], "skipped_invalid_seed");
}

#[test]
fn mcp_stdio_agent_route_uses_backend_fallback() {
    let fixture = fixture_project();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 35,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand invalid explicit seed",
                "files": ["does/not/exist.ts"],
                "token_budget": 1600,
                "force_index": true,
                "backend_evidence": {
                    "provider": "codebase-memory-mcp",
                    "use_as_fallback": true,
                    "candidate_files": ["missing.ts", "src/main.ts", "src/ui.ts"],
                    "evidence_sources": ["search_graph"],
                    "evidence_count": 4
                }
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 35);
    assert!(response["error"].is_null());
    assert_eq!(route["impact_status"], "complete");
    assert_eq!(route["routing_decision"]["first_file"], "src/main.ts");
    assert_eq!(
        route["routing_decision"]["first_seed_source"],
        "backend_fallback"
    );
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_fallback"
    );
    assert_eq!(
        route["routing_decision"]["route_quality"]["recommended_action"],
        "read_backend_seeded_context"
    );
    assert_eq!(
        route["routing_decision"]["continuation_source"],
        "backend_route_agreement"
    );
    assert_eq!(
        route["routing_decision"]["continuation_status"],
        "backend_candidate_available"
    );
    assert_eq!(
        route["routing_decision"]["continuation_next_action"],
        "use_if_fallback_context_insufficient"
    );
    assert_eq!(
        route["execution_plan"][2]["action"],
        route["routing_decision"]["continuation_next_action"]
    );
    assert_eq!(
        route["execution_plan"][2]["suggested_tool"]["tool"],
        "context_pack"
    );
    assert_eq!(
        route["execution_plan"][2]["suggested_tool"]["suggested_arguments"]["files"],
        serde_json::json!(["src/ui.ts"])
    );
    assert_eq!(route["execution_plan"][0]["status"], "ready");
}

#[test]
fn mcp_stdio_agent_route_prefers_backend_context() {
    let fixture = fixture_project();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 36,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand app entrypoint flow",
                "token_budget": 1600,
                "force_index": true,
                "backend_evidence": {
                    "provider": "codebase-memory-mcp",
                    "prefer_for_context": true,
                    "tool_results": {
                        "search_graph": {
                            "structuredContent": {
                                "elapsed_ms": 4,
                                "results": [{
                                    "file_path": "src/ui.ts",
                                    "name": "render",
                                    "label": "Function"
                                }]
                            }
                        }
                    }
                }
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 36);
    assert!(response["error"].is_null());
    assert_eq!(route["routing_decision"]["first_file"], "src/ui.ts");
    assert_eq!(
        route["routing_decision"]["backend_route_agreement"]["status"],
        "backend_preferred"
    );
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["candidate_files"],
        serde_json::json!(["src/ui.ts"])
    );
    assert_eq!(
        route["routing_decision"]["backend_evidence"]["candidates"][0]["symbol"],
        "render"
    );
    assert!(
        route["routing_decision"]["backend_evidence"]
            .get("tool_results")
            .is_none()
    );
    assert_eq!(route["impact_seed_files"], serde_json::json!(["src/ui.ts"]));
    assert_eq!(route["impact_seed_symbols"], serde_json::json!(["render"]));
}

#[test]
fn mcp_stdio_agent_route_returns_blocked_plan_for_unmatched_explicit_symbol() {
    let fixture = fixture_project();
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 34,
        "method": "tools/call",
        "params": {
            "name": "agent_route",
            "arguments": {
                "root": fixture.path(),
                "task": "understand nonexistent explicit symbol",
                "symbols": ["ThisSymbolDoesNotExist"],
                "token_budget": 1000,
                "force_index": true
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();
    let route = &response["result"]["structuredContent"];

    assert_eq!(response["id"], 34);
    assert!(response["error"].is_null());
    assert_eq!(route["route"][2]["status"], "blocked_no_context");
    assert_eq!(route["impact_status"], "skipped_no_context");
    assert_eq!(
        route["context_pack"]["budget"]["truncation_reason"],
        "no_context_for_explicit_seed"
    );
    assert_eq!(
        route["context_pack"]["continuation_summary"]["next_action"],
        "provide_matching_seed_file_or_symbol"
    );
    assert!(route["current_reading_step"].is_null());
    assert_eq!(
        route["execution_plan"][0]["status"],
        "blocked_no_reading_plan"
    );
    assert_eq!(route["execution_plan"][3]["status"], "skipped_no_context");
}

#[test]
fn mcp_stdio_rejects_unknown_method_with_stable_error() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 32,
        "method": "resources/list"
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["id"], 32);
    assert_eq!(response["error"]["code"], -32601);
    assert_eq!(
        response["error"]["message"],
        "method not found: resources/list"
    );
}

#[test]
fn mcp_stdio_rejects_invalid_tool_arguments() {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "symbol_search",
            "arguments": {
                "root": ".",
                "query": "AuthService",
                "limit": 0
            }
        }
    });

    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(["serve", "--transport", "stdio"]);
    command.write_stdin(format!("{request}\n"));
    let output = command.assert().success().get_output().stdout.clone();
    let response: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(response["id"], 2);
    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("limit")
    );
}

fn run_json<const N: usize>(args: [&str; N]) -> Value {
    let output = Command::cargo_bin("codeinsight")
        .unwrap()
        .env_remove("CODEINSIGHT_EMBEDDING_PROVIDER")
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn run_json_with_env<const N: usize, const M: usize>(
    args: [&str; N],
    envs: [(&str, &str); M],
) -> Value {
    let mut command = Command::cargo_bin("codeinsight").unwrap();
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&output).unwrap()
}

fn assert_context_file_has_no_duplicate_lines(file: &Value) {
    let mut seen = std::collections::BTreeSet::new();
    for range in file["ranges"].as_array().unwrap() {
        let start_line = range["start_line"].as_u64().unwrap();
        let end_line = range["end_line"].as_u64().unwrap();
        for line in start_line..=end_line {
            assert!(seen.insert(line), "duplicate context line {line}");
        }
    }
}

fn assert_context_file_ranges_are_sorted(file: &Value) {
    let mut previous_start_line = 0;
    for range in file["ranges"].as_array().unwrap() {
        let start_line = range["start_line"].as_u64().unwrap();
        assert!(
            start_line >= previous_start_line,
            "context ranges are not sorted"
        );
        previous_start_line = start_line;
    }
}

fn assert_context_file_ranges_have_reasons(file: &Value) {
    for range in file["ranges"].as_array().unwrap() {
        assert!(
            !range["reason"].as_str().unwrap_or_default().is_empty(),
            "context range reason is empty"
        );
    }
}

fn assert_agent_route_execution_plan_matches_context(route: &Value) {
    let context_pack = &route["context_pack"];
    let reading_plan = context_pack["reading_plan"].as_array().unwrap();
    assert!(
        !reading_plan.is_empty(),
        "agent_route must return a reading plan"
    );

    let execution_plan = route["execution_plan"].as_array().unwrap();
    assert!(
        execution_plan.len() >= 4,
        "agent_route should expose the full first-read execution plan"
    );

    assert_eq!(
        route["current_reading_step"], reading_plan[0],
        "agent_route should mirror reading_plan[0] at top level for client handoff"
    );
    let routing_decision = &route["routing_decision"];
    assert_eq!(
        routing_decision["seed_strategy"], context_pack["seed_strategy"],
        "routing_decision should mirror context_pack.seed_strategy"
    );
    assert_eq!(
        routing_decision["selected_file_count"].as_u64(),
        Some(context_pack["files"].as_array().unwrap().len() as u64),
        "routing_decision should expose selected file count"
    );
    assert_eq!(
        routing_decision["selected_range_count"], context_pack["budget"]["selected_ranges"],
        "routing_decision should expose selected range count"
    );
    assert_eq!(
        routing_decision["omitted_file_count"], context_pack["budget"]["omitted_files"],
        "routing_decision should expose omitted file count"
    );
    assert_eq!(
        routing_decision["continuation_status"], context_pack["continuation_summary"]["status"],
        "routing_decision should expose continuation status"
    );
    assert_eq!(
        routing_decision["continuation_next_action"],
        context_pack["continuation_summary"]["next_action"],
        "routing_decision should expose continuation next action"
    );
    assert_eq!(
        routing_decision["impact_status"], route["impact_status"],
        "routing_decision should expose impact status"
    );
    if let Some(first_seed) = context_pack["selected_seeds"].as_array().unwrap().first() {
        assert_eq!(
            routing_decision["first_seed_kind"], first_seed["kind"],
            "routing_decision should expose first seed kind"
        );
        assert_eq!(
            routing_decision["first_seed_source"], first_seed["source"],
            "routing_decision should expose first seed source"
        );
        assert_eq!(
            routing_decision["first_seed_value"], first_seed["value"],
            "routing_decision should expose first seed value"
        );
    }
    let selected_source_lines = context_pack["files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| file["ranges"].as_array().unwrap())
        .map(|range| {
            let start = range["start_line"].as_u64().unwrap();
            let end = range["end_line"].as_u64().unwrap();
            end.saturating_sub(start) + 1
        })
        .sum::<u64>();
    let baseline_source_lines = route["overview"]["total_lines"].as_u64().unwrap();
    assert_eq!(
        context_pack["read_less"]["baseline_source_lines"].as_u64(),
        Some(baseline_source_lines),
        "context_pack.read_less should expose the blind first-read baseline"
    );
    assert_eq!(
        routing_decision["baseline_source_lines"].as_u64(),
        Some(baseline_source_lines),
        "routing_decision should expose the blind first-read baseline"
    );
    assert_eq!(
        context_pack["read_less"]["selected_source_lines"].as_u64(),
        Some(selected_source_lines),
        "context_pack.read_less should expose selected source lines"
    );
    assert_eq!(
        routing_decision["selected_source_lines"].as_u64(),
        Some(selected_source_lines),
        "routing_decision should expose selected source lines"
    );
    assert_eq!(
        context_pack["read_less"]["source_lines_avoided"].as_u64(),
        Some(baseline_source_lines.saturating_sub(selected_source_lines)),
        "context_pack.read_less should expose avoided source lines"
    );
    assert_eq!(
        routing_decision["source_lines_avoided"].as_u64(),
        Some(baseline_source_lines.saturating_sub(selected_source_lines)),
        "routing_decision should expose avoided source lines"
    );
    assert!(
        context_pack["read_less"]["line_reduction"]
            .as_str()
            .unwrap()
            .ends_with('%'),
        "context_pack.read_less should expose a line reduction percentage"
    );
    assert_eq!(
        routing_decision["line_reduction"], context_pack["read_less"]["line_reduction"],
        "routing_decision should mirror line reduction"
    );
    assert!(
        context_pack["read_less"]["read_less_ratio"]
            .as_str()
            .unwrap()
            .ends_with('x'),
        "context_pack.read_less should expose a read-less ratio"
    );
    assert_eq!(
        routing_decision["read_less_ratio"], context_pack["read_less"]["read_less_ratio"],
        "routing_decision should mirror read-less ratio"
    );

    let reading_files = reading_plan
        .iter()
        .map(|step| step["file"].as_str().unwrap())
        .collect::<Vec<_>>();
    let execution_read_files = execution_plan[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        execution_read_files, reading_files,
        "read_selected_context should follow reading_plan file order"
    );

    let first_step = &reading_plan[0];
    assert_eq!(
        routing_decision["first_file"], first_step["file"],
        "routing_decision should expose first reading file"
    );
    assert_eq!(
        routing_decision["first_selection_rank"], first_step["selection_rank"],
        "routing_decision should expose first selection rank"
    );
    assert_eq!(
        routing_decision["first_focus"], first_step["focus"],
        "routing_decision should expose first focus"
    );
    assert_eq!(
        routing_decision["first_question"], first_step["question"],
        "routing_decision should expose first question"
    );
    assert_eq!(
        routing_decision["first_next_action"], first_step["next_action"],
        "routing_decision should expose first next action"
    );
    assert_eq!(
        routing_decision["first_selection_reason"], first_step["selection_reason"],
        "routing_decision should expose first selection reason"
    );
    assert_eq!(
        routing_decision["first_suggested_tool"], first_step["suggested_tool"],
        "routing_decision should expose first suggested tool"
    );
    assert_eq!(execution_plan[0]["status"], "ready");
    assert!(
        execution_plan[0]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["file"].as_str().unwrap()),
        "first execution step should name the first reading-plan file"
    );
    assert!(
        execution_plan[0]["instruction"]
            .as_str()
            .unwrap()
            .contains(&format!(
                "candidate rank {}",
                first_step["selection_rank"].as_u64().unwrap()
            )),
        "first execution step should expose the first reading-plan candidate rank"
    );
    assert!(
        execution_plan[0]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["focus"].as_str().unwrap()),
        "first execution step should include the first reading-plan focus"
    );
    assert!(
        execution_plan[0]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["question"].as_str().unwrap()),
        "first execution step should include the first reading-plan question"
    );
    let first_instruction = execution_plan[0]["instruction"].as_str().unwrap();
    assert!(
        first_instruction.contains("Read-less evidence: selected"),
        "first execution step should expose read-less evidence"
    );
    assert!(
        first_instruction.contains(&format!(
            "selected {} of {} source lines",
            context_pack["read_less"]["selected_source_lines"]
                .as_u64()
                .unwrap(),
            context_pack["read_less"]["baseline_source_lines"]
                .as_u64()
                .unwrap()
        )),
        "first execution step should expose selected and baseline source lines"
    );
    assert!(
        first_instruction.contains(&format!(
            "avoided {}",
            context_pack["read_less"]["source_lines_avoided"]
                .as_u64()
                .unwrap()
        )),
        "first execution step should expose avoided source lines"
    );
    assert!(
        first_instruction.contains(
            context_pack["read_less"]["read_less_ratio"]
                .as_str()
                .unwrap()
        ),
        "first execution step should expose read-less ratio"
    );
    assert_eq!(execution_plan[1]["files"][0], first_step["file"]);
    assert_eq!(
        execution_plan[1]["suggested_tool"], first_step["suggested_tool"],
        "current-step suggested tool should mirror reading_plan[0]"
    );
    assert!(
        execution_plan[1]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["suggested_tool"]["tool"].as_str().unwrap()),
        "current-step instruction should name the suggested tool"
    );
    assert!(
        execution_plan[1]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["next_action"].as_str().unwrap()),
        "current-step instruction should name the reading-plan action"
    );
    assert!(
        execution_plan[1]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["focus"].as_str().unwrap()),
        "current-step instruction should include the reading-plan focus"
    );
    assert!(
        execution_plan[1]["instruction"]
            .as_str()
            .unwrap()
            .contains(first_step["question"].as_str().unwrap()),
        "current-step instruction should include the reading-plan question"
    );

    let continuation = &context_pack["continuation_summary"];
    assert!(
        execution_plan[2]["instruction"]
            .as_str()
            .unwrap()
            .contains(continuation["next_action"].as_str().unwrap()),
        "continuation execution step should name continuation_summary.next_action"
    );
    if continuation["status"] == "complete" {
        assert!(
            execution_plan[2]["instruction"]
                .as_str()
                .unwrap()
                .contains("no follow-up tool is required after selected context"),
            "complete continuation step should explain that no extra continuation tool is required"
        );
    }

    let omitted_candidates = context_pack["omitted_candidates"].as_array().unwrap();
    if let Some(first_omitted) = omitted_candidates.first() {
        let continuation_instruction = execution_plan[2]["instruction"].as_str().unwrap();
        assert!(
            continuation_instruction.contains(first_omitted["file"].as_str().unwrap()),
            "continuation execution step should name the first omitted candidate"
        );
        assert!(
            continuation_instruction.contains(&format!(
                "candidate rank {}",
                first_omitted["selection_rank"].as_u64().unwrap()
            )),
            "continuation execution step should expose the omitted candidate rank"
        );
        assert!(
            continuation_instruction.contains(first_omitted["omission_reason"].as_str().unwrap()),
            "continuation execution step should expose the omission reason"
        );
    }

    if !continuation["suggested_tool"].is_null() {
        assert_eq!(
            execution_plan[2]["status"],
            "available_after_selected_context"
        );
        assert_eq!(
            execution_plan[2]["suggested_tool"], continuation["suggested_tool"],
            "continuation suggested tool should mirror continuation_summary"
        );
    }

    if !route["impact_analysis"].is_null() {
        let impact_step = &execution_plan[3];
        assert_eq!(impact_step["action"], "review_impact_before_edits");
        assert_eq!(impact_step["status"], route["impact_status"]);
        assert_eq!(
            impact_step["suggested_checks"], route["impact_analysis"]["suggested_checks"],
            "impact execution step should mirror impact_analysis.suggested_checks"
        );
        assert_eq!(
            impact_step["suggested_tool"]["tool"], "impact_analysis",
            "impact execution step should expose the impact_analysis follow-up tool"
        );
        assert_eq!(
            impact_step["suggested_tool"]["suggested_arguments"]["root"], route["root"],
            "impact suggested tool should preserve the route root"
        );
        assert_eq!(
            impact_step["suggested_tool"]["suggested_arguments"]["files"],
            route["impact_seed_files"],
            "impact suggested tool should preserve impact seed files"
        );
        assert!(
            impact_step["instruction"]
                .as_str()
                .unwrap()
                .contains("First suggested check:"),
            "impact execution step should name the first suggested check"
        );
        if let Some(first_check) = route["impact_analysis"]["suggested_checks"]
            .as_array()
            .unwrap()
            .first()
            && let Some(command) = first_check["command"].as_str()
        {
            assert!(
                impact_step["instruction"]
                    .as_str()
                    .unwrap()
                    .contains(command),
                "impact execution step should include the first command check"
            );
        }
    }
}

fn fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    write_file(
        &dir,
        "package.json",
        r##"
{
  "name": "fixture-lib",
  "workspaces": ["packages/*"],
  "imports": {
    "#internal/*": "./src/internal/*.ts",
    "#internal/special": "./src/import-special.ts",
    "#internal/special/*": "./src/import-special/*.ts",
    "#fallback/*": ["./src/missing/*.ts", "./src/internal/*.ts"],
    "#multi/*/component/*": "./src/multi/*/component/*.ts"
  },
  "exports": {
    "./package-*": "./src/package-*.ts",
    "./multi/*/component/*": "./src/multi/*/component/*.ts"
  }
}
"##,
    );
    write_file(
        &dir,
        "tsconfig.base.json",
        r#"
{
  "compilerOptions": {
    "baseUrl": "src",
    "paths": {
      "@base/*": ["base/*"]
    }
  }
}
"#,
    );
    write_file(
        &dir,
        "tsconfig.json",
        r#"
{
  "extends": "./tsconfig.base.json",
  "compilerOptions": {
    "baseUrl": "src",
    "paths": {
      "@app/*": ["*"],
      "@app/special": ["path-special.ts"],
      "@app/special/*": ["path-special/*.ts"],
      "@fallback/*": ["missing/*", "*"],
      "@multi/*/component/*": ["multi/*/component/*.ts"]
    }
  }
}
"#,
    );
    write_file(
        &dir,
        "src/feature/tsconfig.json",
        r#"
{
  "extends": "../../tsconfig.base.json"
}
"#,
    );

    write_file(
        &dir,
        "src/auth.py",
        r#"
import os

class AuthService:
    def login(self):
        return helper()

def helper():
    return os.getenv("USER")
"#,
    );
    write_file(
        &dir,
        "src/billing.py",
        r#"
class BillingService:
    def charge(self):
        return "paid"
"#,
    );
    write_file(
        &dir,
        "src/consumer.py",
        r#"
from auth import AuthService

def build_service():
    return AuthService()
"#,
    );
    write_file(
        &dir,
        "src/auth_notes.py",
        r#"
# Session cookie behavior note.
# Refresh cookie expiry should stay aligned with login state.
"#,
    );
    write_file(
        &dir,
        "src/main.ts",
        r##"
import { render } from "./ui";
import drawDefault from "./ui";
import { relayRender, relayDefault, render as starRender, uiApi } from "./barrel";
import { finalApi, finalDefault, finalRender } from "./barrel2";
import * as ui from "./ui";
import { pathRender } from "@app/path-ui";
import { specialPathRender } from "@app/special";
import { specialButtonPathRender } from "@app/special/button";
import { fallbackRender } from "@fallback/fallback-ui";
import { sharedRender } from "shared";
import { packageRender } from "fixture-lib/package-ui";
import { depRender } from "dep-lib/feature";
import { depArrayRender } from "dep-lib/array-feature";
import { depNodeRender } from "dep-lib/node-feature";
import { browserRootRender } from "browser-lib";
import { browserExternalRootRender } from "browser-external-lib";
import { browserServerRender } from "browser-object-lib/server";
import { browserPlainRender } from "browser-object-lib/plain";
import { browserExternalRender } from "browser-object-lib/external";
import { browserAbsoluteRender } from "browser-object-lib/absolute";
import { browserObjectRender } from "browser-object-lib/object";
import { browserDisabledRender } from "browser-object-lib/disabled";
import { legacyRender } from "legacy-lib";
import { legacyPluginRender } from "legacy-lib/plugin";
import { rootArrayRender } from "root-array-lib";
import { rootBrowserExportRender } from "root-browser-export-lib";
import { workspaceButton } from "workspace-ui/button";
import { logInternal } from "#internal/logger";
import { specialInternalRender } from "#internal/special";
import { specialInternalButtonRender } from "#internal/special/button";
import { logInternal as logFallback } from "#fallback/logger";
import { multiPathRender } from "@multi/admin/component/card";
import { multiPackageRender } from "fixture-lib/multi/admin/component/card";
import { multiInternalRender } from "#multi/admin/component/card";
const { render: draw } = require("./ui");
const uiModule = require("./ui");
const computedUiModule = require("./" + "ui");
const modalPath = "./ui";
const modalModule = require(modalPath);
const modalLoaded = await import(modalPath);
const templateRoot = "./";
const templatePath = `${templateRoot}ui`;
const templateModule = require(templatePath);

export function main() {
  render();
}

export function aliasMain() {
  draw();
}

export function namespaceMain() {
  ui.render();
}

export function moduleAliasMain() {
  uiModule.render();
}

export function computedModuleAliasMain() {
  computedUiModule.render();
}

export function variableModuleAliasMain() {
  modalModule.render();
}

export async function variableDynamicImportMain() {
  modalLoaded.render();
}

export function defaultMain() {
  drawDefault();
}

export function reexportMain() {
  relayRender();
}

export function reexportDefaultMain() {
  relayDefault();
}

export function exportStarMain() {
  starRender();
}

export function namespaceReexportMain() {
  uiApi.render();
}

export function twoHopReexportMain() {
  finalRender();
}

export function twoHopDefaultMain() {
  finalDefault();
}

export function twoHopNamespaceMain() {
  finalApi.render();
}

export function requireMemberMain() {
  require("./ui").render();
}

export function computedRequireMemberMain() {
  require("./" + "ui").render();
}

export function variableRequireMemberMain() {
  require(modalPath).render();
}

export function variableTemplateRequireMemberMain() {
  require(templatePath).render();
}

export async function dynamicImportMain() {
  const loadedUi = await import("./ui");
  loadedUi.render();
}

export function dynamicImportThenMain() {
  import("./ui").then((thenUi) => {
    thenUi.render();
  });
}

export function pathAliasMain() {
  pathRender();
}

export function pathAliasPrecedenceMain() {
  specialPathRender();
  specialButtonPathRender();
}

export function fallbackAliasMain() {
  fallbackRender();
}

export function baseUrlIndexMain() {
  sharedRender();
}

export function packageExportMain() {
  packageRender();
}

export function dependencyPackageMain() {
  depRender();
  depArrayRender();
  depNodeRender();
  browserRootRender();
  browserExternalRootRender();
  browserServerRender();
  browserPlainRender();
  browserExternalRender();
  browserAbsoluteRender();
  browserObjectRender();
  browserDisabledRender();
  legacyRender();
  legacyPluginRender();
  rootArrayRender();
  rootBrowserExportRender();
}

export function workspacePackageMain() {
  workspaceButton();
}

export function packageImportMain() {
  logInternal();
  specialInternalRender();
  specialInternalButtonRender();
  logFallback();
}

export function multiWildcardMain() {
  multiPathRender();
  multiPackageRender();
  multiInternalRender();
}
"##,
    );
    write_file(
        &dir,
        "src/metadata-entry.ts",
        r#"
import { invalidMetadataRender } from "metadata-invalid-lib";

export function metadataEntryMain() {
  invalidMetadataRender();
}
"#,
    );
    write_file(
        &dir,
        "src/feature/entry.ts",
        r#"
import { baseRender } from "@base/base-ui";

export function inheritedPathsMain() {
  baseRender();
}
"#,
    );
    write_file(
        &dir,
        "src/call-entry.ts",
        r#"
import { relayRender } from "./barrel";

export function callGraphEntry() {
  relayRender();
}
"#,
    );
    write_file(
        &dir,
        "node_modules/dep-lib/package.json",
        r#"
{
  "name": "dep-lib",
  "exports": {
    "./feature": {
      "import": "./dist/feature.js"
    },
    "./array-feature": {
      "import": [
        "./dist/missing-array-feature.js",
        "./dist/array-feature.js"
      ]
    },
    "./node-feature": {
      "node": {
        "import": "./dist/node-feature.js"
      },
      "default": "./dist/default-feature.js"
    }
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/workspace-ui/package.json",
        r#"
{
  "name": "workspace-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/workspace-ui/src/button.ts",
        r#"
export function workspaceButton() {
  return "workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-array-lib/package.json",
        r#"
{
  "name": "root-array-lib",
  "exports": [
    null,
    "external-root",
    "./dist/missing-index.js",
    "./dist/index.js"
  ]
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-array-lib/dist/index.js",
        r#"
export function rootArrayRender() {
  return "root-array";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-array-lib/external-root.js",
        r#"
export function rootArrayRender() {
  return "external-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-browser-export-lib/package.json",
        r#"
{
  "name": "root-browser-export-lib",
  "exports": {
    ".": {
      "import": "./dist/node.js"
    }
  },
  "browser": "./dist/browser.js"
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-browser-export-lib/dist/browser.js",
        r#"
export function rootBrowserExportRender() {
  return "browser-export-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/root-browser-export-lib/dist/node.js",
        r#"
export function rootBrowserExportRender() {
  return "node-export-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-lib/package.json",
        r#"
{
  "name": "browser-lib",
  "main": "./dist/node-entry.js",
  "browser": "./dist/browser-entry.js"
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-lib/dist/browser-entry.js",
        r#"
export function browserRootRender() {
  return "browser-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-lib/dist/node-entry.js",
        r#"
export function browserRootRender() {
  return "node-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-external-lib/package.json",
        r#"
{
  "name": "browser-external-lib",
  "main": "./dist/node-entry.js",
  "browser": "external-browser-entry"
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-external-lib/dist/node-entry.js",
        r#"
export function browserExternalRootRender() {
  return "node-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-external-lib/external-browser-entry.js",
        r#"
export function browserExternalRootRender() {
  return "external-root";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/package.json",
        r#"
{
  "name": "browser-object-lib",
  "exports": {
    "./server": "./dist/server.js",
    "./plain": "./dist/plain.js",
    "./external": "./dist/external.js",
    "./absolute": "./dist/absolute.js",
    "./object": "./dist/object.js",
    "./disabled": "./dist/disabled.js"
  },
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
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/browser-server.js",
        r#"
export function browserServerRender() {
  return "browser-server";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/browser-plain.js",
        r#"
export function browserPlainRender() {
  return "browser-plain";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/server.js",
        r#"
export function browserServerRender() {
  return "node-server";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/external.js",
        r#"
export function browserExternalRender() {
  return "node-external";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/browser-absolute.js",
        r#"
export function browserAbsoluteRender() {
  return "browser-absolute";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/absolute.js",
        r#"
export function browserAbsoluteRender() {
  return "node-absolute";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/browser-object.js",
        r#"
export function browserObjectRender() {
  return "browser-object";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/object.js",
        r#"
export function browserObjectRender() {
  return "node-object";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/external-browser-shim.js",
        r#"
export function browserExternalRender() {
  return "external-shim";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/browser-object-lib/dist/disabled.js",
        r#"
export function browserDisabledRender() {
  return "node-disabled";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/legacy-lib/package.json",
        r#"
{
  "name": "legacy-lib",
  "module": "./dist/index.js",
  "main": "./dist/cjs.js"
}
"#,
    );
    write_file(
        &dir,
        "node_modules/legacy-lib/dist/index.js",
        r#"
export function legacyRender() {
  return "legacy";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/metadata-invalid-lib/package.json",
        r#"
{
  "name": "metadata-invalid-lib",
  "module": "external-entry",
  "main": "/dist/absolute-entry.js",
  "types": {
    "default": "./dist/index.d.ts"
  },
  "typings": false
}
"#,
    );
    write_file(
        &dir,
        "node_modules/metadata-invalid-lib/external-entry.js",
        r#"
export function invalidMetadataRender() {
  return "external";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/metadata-invalid-lib/dist/absolute-entry.js",
        r#"
export function invalidMetadataRender() {
  return "absolute";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/legacy-lib/plugin/index.js",
        r#"
export function legacyPluginRender() {
  return "legacy-plugin";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/dep-lib/dist/feature.js",
        r#"
export function depRender() {
  return "dep";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/dep-lib/dist/array-feature.js",
        r#"
export function depArrayRender() {
  return "dep-array";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/dep-lib/dist/node-feature.js",
        r#"
export function depNodeRender() {
  return "node";
}
"#,
    );
    write_file(
        &dir,
        "src/barrel.ts",
        r#"
export { render as relayRender, default as relayDefault } from "./ui";
export * from "./ui";
export * as uiApi from "./ui";
"#,
    );
    write_file(
        &dir,
        "src/barrel2.ts",
        r#"
export { relayRender as finalRender, relayDefault as finalDefault, uiApi as finalApi } from "./barrel";
"#,
    );
    write_file(
        &dir,
        "src/ui.ts",
        r#"
export function render() {
  return "ok";
}

export default function defaultRender() {
  return "default";
}
"#,
    );
    write_file(
        &dir,
        "src/path-ui.ts",
        r#"
export function pathRender() {
  return "path";
}
"#,
    );
    write_file(
        &dir,
        "src/path-special.ts",
        r#"
export function specialPathRender() {
  return "path-special";
}
"#,
    );
    write_file(
        &dir,
        "src/path-special/button.ts",
        r#"
export function specialButtonPathRender() {
  return "path-special-button";
}
"#,
    );
    write_file(
        &dir,
        "src/special.ts",
        r#"
export function specialPathRender() {
  return "broad-special";
}
"#,
    );
    write_file(
        &dir,
        "src/special/button.ts",
        r#"
export function specialButtonPathRender() {
  return "broad-special-button";
}
"#,
    );
    write_file(
        &dir,
        "src/fallback-ui.ts",
        r#"
export function fallbackRender() {
  return "fallback";
}
"#,
    );
    write_file(
        &dir,
        "src/shared/index.ts",
        r#"
export function sharedRender() {
  return "shared";
}
"#,
    );
    write_file(
        &dir,
        "src/base/base-ui.ts",
        r#"
export function baseRender() {
  return "base";
}
"#,
    );
    write_file(
        &dir,
        "src/package-ui.ts",
        r#"
export function packageRender() {
  return "package";
}
"#,
    );
    write_file(
        &dir,
        "src/internal/logger.ts",
        r#"
export function logInternal() {
  return "log";
}
"#,
    );
    write_file(
        &dir,
        "src/import-special.ts",
        r#"
export function specialInternalRender() {
  return "import-special";
}
"#,
    );
    write_file(
        &dir,
        "src/import-special/button.ts",
        r#"
export function specialInternalButtonRender() {
  return "import-special-button";
}
"#,
    );
    write_file(
        &dir,
        "src/internal/special.ts",
        r#"
export function specialInternalRender() {
  return "broad-import-special";
}
"#,
    );
    write_file(
        &dir,
        "src/internal/special/button.ts",
        r#"
export function specialInternalButtonRender() {
  return "broad-import-special-button";
}
"#,
    );
    write_file(
        &dir,
        "src/multi/admin/component/card.ts",
        r#"
export function multiPathRender() {
  return "multi-path";
}

export function multiPackageRender() {
  return "multi-package";
}

export function multiInternalRender() {
  return "multi-internal";
}
"#,
    );
    write_file(&dir, "src/long.ts", &long_typescript_file());
    write_file(&dir, "src/huge.ts", &huge_typescript_file());
    write_file(&dir, "src/multi-long.ts", &multi_long_typescript_file());
    write_file(
        &dir,
        "src/service.go",
        r#"
package service

import "fmt"

func Login() {
  fmt.Println("login")
}
"#,
    );

    dir
}

fn framework_entrypoint_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "app/page.tsx",
        r#"
export default function Page() {
  return <main>Dashboard</main>;
}
"#,
    );
    write_file(
        &dir,
        "pages/_app.tsx",
        r#"
export default function App({ Component, pageProps }) {
  return <Component {...pageProps} />;
}
"#,
    );
    write_file(
        &dir,
        "config/routes.rb",
        r#"
Rails.application.routes.draw do
  root "dashboard#index"
end
"#,
    );
    write_file(
        &dir,
        "src/BillingApplication.java",
        r#"
package fixture;

public class BillingApplication {
}
"#,
    );
    write_file(
        &dir,
        "manage.py",
        r#"
from django.core.management import execute_from_command_line

if __name__ == "__main__":
    execute_from_command_line()
"#,
    );
    write_file(
        &dir,
        "project/asgi.py",
        r#"
from django.core.asgi import get_asgi_application

application = get_asgi_application()
"#,
    );
    write_file(
        &dir,
        "project/wsgi.py",
        r#"
from django.core.wsgi import get_wsgi_application

application = get_wsgi_application()
"#,
    );
    write_file(
        &dir,
        "project/urls.py",
        r#"
from django.urls import path

urlpatterns = [
    path("", lambda request: None),
]
"#,
    );
    write_file(
        &dir,
        "src/Program.cs",
        r#"
var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();
app.Run();
"#,
    );
    write_file(
        &dir,
        "src/Startup.cs",
        r#"
public class Startup
{
    public void Configure()
    {
    }
}
"#,
    );

    dir
}

fn pnpm_workspace_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "pnpm-workspace.yaml",
        r#"
packages:
  - "packages/**"
  - "!packages/legacy/**"
catalog:
  default-catalog-ui: ^1.0.0
catalogs:
  react18:
    catalog-ui: ^1.2.3
"#,
    );
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "version-star-ui": "workspace:*",
    "version-caret-ui": "workspace:^",
    "version-tilde-ui": "workspace:~",
    "version-exact-ui": "workspace:1.2.3",
    "deep-ui": "workspace:*",
    "catalog-ui": "catalog:react18",
    "default-catalog-ui": "catalog:",
    "legacy-ui": "workspace:*"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { pnpmButton } from "pnpm-ui/button";
import { versionStarButton } from "version-star-ui/button";
import { versionCaretButton } from "version-caret-ui/button";
import { versionTildeButton } from "version-tilde-ui/button";
import { versionExactButton } from "version-exact-ui/button";
import { deepButton } from "deep-ui/button";
import { catalogButton } from "catalog-ui/button";
import { defaultCatalogButton } from "default-catalog-ui/button";
import { legacyButton } from "legacy-ui/button";

export function pnpmWorkspaceMain() {
  pnpmButton();
}

export function pnpmWorkspaceVersionMain() {
  versionStarButton();
  versionCaretButton();
  versionTildeButton();
  versionExactButton();
  deepButton();
  catalogButton();
  defaultCatalogButton();
  legacyButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/pnpm-ui/package.json",
        r#"
{
  "name": "pnpm-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/pnpm-ui/src/button.ts",
        r#"
export function pnpmButton() {
  return "pnpm";
}
"#,
    );
    write_file(
        &dir,
        "packages/version-star-ui/package.json",
        r#"
{
  "name": "version-star-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/version-star-ui/src/button.ts",
        r#"
export function versionStarButton() {
  return "star";
}
"#,
    );
    write_file(
        &dir,
        "packages/version-caret-ui/package.json",
        r#"
{
  "name": "version-caret-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/version-caret-ui/src/button.ts",
        r#"
export function versionCaretButton() {
  return "caret";
}
"#,
    );
    write_file(
        &dir,
        "packages/version-tilde-ui/package.json",
        r#"
{
  "name": "version-tilde-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/version-tilde-ui/src/button.ts",
        r#"
export function versionTildeButton() {
  return "tilde";
}
"#,
    );
    write_file(
        &dir,
        "packages/version-exact-ui/package.json",
        r#"
{
  "name": "version-exact-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/version-exact-ui/src/button.ts",
        r#"
export function versionExactButton() {
  return "exact";
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-ui/package.json",
        r#"
{
  "name": "deep-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-ui/src/button.ts",
        r#"
export function deepButton() {
  return "deep";
}
"#,
    );
    write_file(
        &dir,
        "packages/catalog-ui/package.json",
        r#"
{
  "name": "catalog-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/catalog-ui/src/button.ts",
        r#"
export function catalogButton() {
  return "catalog";
}
"#,
    );
    write_file(
        &dir,
        "packages/default-catalog-ui/package.json",
        r#"
{
  "name": "default-catalog-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/default-catalog-ui/src/button.ts",
        r#"
export function defaultCatalogButton() {
  return "default-catalog-workspace";
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/legacy-ui/package.json",
        r#"
{
  "name": "legacy-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/legacy-ui/src/button.ts",
        r#"
export function legacyButton() {
  return "legacy-workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/catalog-ui/package.json",
        r#"
{
  "name": "catalog-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/catalog-ui/dist/button.js",
        r#"
export function catalogButton() {
  return "catalog-node";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/default-catalog-ui/package.json",
        r#"
{
  "name": "default-catalog-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/default-catalog-ui/dist/button.js",
        r#"
export function defaultCatalogButton() {
  return "default-catalog-node";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/legacy-ui/package.json",
        r#"
{
  "name": "legacy-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/legacy-ui/dist/button.js",
        r#"
export function legacyButton() {
  return "legacy-node";
}
"#,
    );
    dir
}

fn workspace_protocol_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "protocol-ui": "workspace:../../packages/protocol-ui"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { protocolButton } from "protocol-ui/button";
import { protocolSpecial } from "protocol-ui/feature/special";
import { protocolSpecialButton } from "protocol-ui/feature/special/button";

export function workspaceProtocolMain() {
  protocolButton();
  protocolSpecial();
  protocolSpecialButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/package.json",
        r#"
{
  "name": "internal-ui",
  "exports": {
    "./button": "./src/button.ts",
    "./feature/*": "./src/feature/*.ts",
    "./feature/special": "./src/feature-special.ts",
    "./feature/special/*": "./src/feature-special/*.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/src/button.ts",
        r#"
export function protocolButton() {
  return "protocol";
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/src/feature-special.ts",
        r#"
export function protocolSpecial() {
  return "protocol-special";
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/src/feature-special/button.ts",
        r#"
export function protocolSpecialButton() {
  return "protocol-special-button";
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/src/feature/special.ts",
        r#"
export function protocolSpecial() {
  return "broad-protocol-special";
}
"#,
    );
    write_file(
        &dir,
        "packages/protocol-ui/src/feature/special/button.ts",
        r#"
export function protocolSpecialButton() {
  return "broad-protocol-special-button";
}
"#,
    );
    dir
}

fn null_package_exports_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/main.ts",
        r#"
import { enabledRender } from "null-export-lib/enabled";
import { arrayRender } from "null-export-lib/array";
import { disabledRender } from "null-export-lib/disabled";
import { conditionalRender } from "null-export-lib/conditional";
import { conditionalExternalRender } from "null-export-lib/conditional-external";

export function nullExportMain() {
  enabledRender();
  arrayRender();
  disabledRender();
  conditionalRender();
  conditionalExternalRender();
}
"#,
    );
    write_file(
        &dir,
        "package.json",
        r#"
{
  "name": "null-export-lib",
  "exports": {
    "./enabled": "./src/enabled.ts",
    "./array": [
      null,
      "external-export-lib",
      "./src/array-fallback.ts"
    ],
    "./disabled": null,
    "./conditional": {
      "import": null,
      "default": "./src/conditional-fallback.ts"
    },
    "./conditional-external": {
      "import": "external-export-lib",
      "default": "./src/conditional-external-fallback.ts"
    }
  }
}
"#,
    );
    write_file(
        &dir,
        "src/enabled.ts",
        r#"
export function enabledRender() {
  return "enabled";
}
"#,
    );
    write_file(
        &dir,
        "src/array-fallback.ts",
        r#"
export function arrayRender() {
  return "array-fallback";
}
"#,
    );
    write_file(
        &dir,
        "external-export-lib.ts",
        r#"
export function arrayRender() {
  return "external-export";
}

export function conditionalExternalRender() {
  return "external-conditional";
}
"#,
    );
    write_file(
        &dir,
        "src/disabled.ts",
        r#"
export function disabledRender() {
  return "disabled";
}
"#,
    );
    write_file(
        &dir,
        "src/conditional-fallback.ts",
        r#"
export function conditionalRender() {
  return "conditional";
}
"#,
    );
    write_file(
        &dir,
        "src/conditional-external-fallback.ts",
        r#"
export function conditionalExternalRender() {
  return "conditional-external";
}
"#,
    );
    dir
}

fn package_subpath_fallback_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/main.ts",
        r#"
import { fileRender } from "subpath-fallback-lib/file";
import { indexRender } from "subpath-fallback-lib/dir";
import { extensionlessRender } from "extensionless-export-lib/feature";
import { specialRender } from "wildcard-precedence-lib/feature/special";
import { specialButtonRender } from "wildcard-precedence-lib/feature/special/button";
import { missingRender } from "subpath-fallback-lib/missing";
import { disabledRender } from "subpath-disabled-lib/disabled";

export function packageSubpathFallbackMain() {
  fileRender();
  indexRender();
  extensionlessRender();
  specialRender();
  specialButtonRender();
  missingRender();
  disabledRender();
}
"#,
    );
    write_file(
        &dir,
        "node_modules/subpath-fallback-lib/package.json",
        r#"
{
  "name": "subpath-fallback-lib"
}
"#,
    );
    write_file(
        &dir,
        "node_modules/subpath-fallback-lib/file.js",
        r#"
export function fileRender() {
  return "file";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/subpath-fallback-lib/dir/index.js",
        r#"
export function indexRender() {
  return "index";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/extensionless-export-lib/package.json",
        r#"
{
  "name": "extensionless-export-lib",
  "exports": {
    "./feature": "./dist/feature"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/extensionless-export-lib/dist/feature.js",
        r#"
export function extensionlessRender() {
  return "extensionless";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/wildcard-precedence-lib/package.json",
        r#"
{
  "name": "wildcard-precedence-lib",
  "exports": {
    "./feature/*": "./dist/wildcard/*.js",
    "./feature/special": "./dist/special.js",
    "./feature/special/*": "./dist/special/*.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/wildcard-precedence-lib/dist/special.js",
        r#"
export function specialRender() {
  return "special";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/wildcard-precedence-lib/dist/special/button.js",
        r#"
export function specialButtonRender() {
  return "special-button";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/wildcard-precedence-lib/dist/wildcard/special.js",
        r#"
export function specialRender() {
  return "wildcard-special";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/subpath-disabled-lib/package.json",
        r#"
{
  "name": "subpath-disabled-lib",
  "exports": {
    "./disabled": null
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/subpath-disabled-lib/disabled.js",
        r#"
export function disabledRender() {
  return "disabled";
}
"#,
    );
    dir
}

fn c_like_include_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/auth.c",
        r#"
#include "auth.h"
#include <stdio.h>

int login(void) {
  return AUTH_OK;
}
"#,
    );
    write_file(
        &dir,
        "src/auth.h",
        r#"
#define AUTH_OK 1
"#,
    );
    write_file(
        &dir,
        "src/service.cpp",
        r#"
#include "include/shared.hpp"

int service(void) {
  return shared_value();
}
"#,
    );
    write_file(
        &dir,
        "src/client.cpp",
        r#"
#include "../include/shared.hpp"

int client(void) {
  return shared_value() + declared_value();
}
"#,
    );
    write_file(
        &dir,
        "include/shared.hpp",
        r#"
inline int shared_value() {
  return 1;
}

int declared_value(void);
"#,
    );
    dir
}

fn go_module_import_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "go.mod",
        r#"
module github.com/example/codeinsight

go 1.22
"#,
    );
    write_file(
        &dir,
        "cmd/server/main.go",
        r#"
package main

import (
  "fmt"

  "github.com/acme/remote"
  "github.com/example/codeinsight/internal/auth"
  cfg "github.com/example/codeinsight/internal/config"
  "github.com/example/codeinsight/internal/metrics"
)

func main() {
  fmt.Println(remote.Name, auth.Login(), cfg.Load(), metrics.Track())
}
"#,
    );
    write_file(
        &dir,
        "internal/auth/service.go",
        r#"
package auth

func Login() string {
  return "ok"
}
"#,
    );
    write_file(
        &dir,
        "internal/config/config.go",
        r#"
package config

func Load() string {
  return "local"
}
"#,
    );
    write_file(
        &dir,
        "internal/metrics/doc.go",
        r#"
package metrics

// Package metrics records runtime counters.
"#,
    );
    write_file(
        &dir,
        "internal/metrics/metrics.go",
        r#"
package metrics

func Track() string {
  return "tracked"
}
"#,
    );
    dir
}

fn java_source_import_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/main/java/com/example/app/App.java",
        r#"
package com.example.app;

import com.acme.RemoteClient;
import com.example.auth.AuthService;
import com.example.reporting.*;
import static com.example.util.Names.defaultName;
import java.util.List;

public class App extends BaseApp {
    private final List<String> names;

    public App(List<String> names) {
        this.names = names;
    }

    public String run(RemoteClient remote) {
        Report.log();
        return LocalFormatter.decorate(AuthService.login(defaultName(), names.size(), remote.id()));
    }
}
"#,
    );
    write_file(
        &dir,
        "src/main/java/com/example/app/BaseApp.java",
        r#"
package com.example.app;

public class BaseApp {
    protected String bootName() {
        return "app";
    }
}
"#,
    );
    write_file(
        &dir,
        "src/main/java/com/example/app/LocalFormatter.java",
        r#"
package com.example.app;

public class LocalFormatter {
    public static String decorate(String name) {
        return name.trim();
    }
}
"#,
    );
    write_file(
        &dir,
        "src/main/java/com/example/auth/AuthService.java",
        r#"
package com.example.auth;

public class AuthService {
    public static String login(String name, int count, String remoteId) {
        return name + count + remoteId;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/main/java/com/example/util/Names.java",
        r#"
package com.example.util;

public class Names {
    public static String defaultName() {
        return "guest";
    }
}
"#,
    );
    write_file(
        &dir,
        "src/main/java/com/example/reporting/Report.java",
        r#"
package com.example.reporting;

public class Report {
    public static void log() {
    }
}
"#,
    );
    dir
}

fn php_namespace_use_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/Controller/AuthController.php",
        r#"<?php
namespace App\Controller;

use App\Repository\UserRepository;
use App\Support\AuditLog;
use App\Support\{Metrics as MetricsAlias};
use function App\Support\audit_login;
use function App\Support\{audit_event as event};
use Vendor\Package\RemoteClient;

class AuthController
{
    public function __construct(private UserRepository $users) {}

    public function login(RemoteClient $remote): bool
    {
        AuditLog::record($remote->id());
        audit_login($remote->id());
        MetricsAlias::track();
        event($remote->id());
        return $this->users->exists($remote->id());
    }
}
"#,
    );
    write_file(
        &dir,
        "src/Repository/UserRepository.php",
        r#"<?php
namespace App\Repository;

class UserRepository
{
    public function exists(string $id): bool
    {
        return $id !== '';
    }
}
"#,
    );
    write_file(
        &dir,
        "src/Support/AuditLog.php",
        r#"<?php
namespace App\Support;

class AuditLog
{
    public static function record(string $id): void
    {
    }
}
"#,
    );
    write_file(
        &dir,
        "src/Support/audit_login.php",
        r#"<?php
namespace App\Support;

function audit_login(string $id): void
{
}
"#,
    );
    write_file(
        &dir,
        "src/Support/Metrics.php",
        r#"<?php
namespace App\Support;

class Metrics
{
    public static function track(): void
    {
    }
}
"#,
    );
    write_file(
        &dir,
        "src/Support/audit_event.php",
        r#"<?php
namespace App\Support;

function audit_event(string $id): void
{
}
"#,
    );
    dir
}

fn ruby_require_relative_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "lib/auth_service.rb",
        r#"
require "json"
require_relative "support/audit"

module Example
  class AuthService
    def login(id)
      Audit.record(id)
      JSON.generate(id: id)
    end
  end
end
"#,
    );
    write_file(
        &dir,
        "lib/services/runner.rb",
        r#"
require_relative "../support/audit.rb"

module Example
  module Services
    class Runner
      def run(id)
        Audit.record(id)
      end
    end
  end
end
"#,
    );
    write_file(
        &dir,
        "lib/support/audit.rb",
        r#"
module Example
  module Audit
    def self.record(id)
      id
    end
  end
end
"#,
    );
    dir
}

fn csharp_using_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/AuthController.cs",
        r#"
using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using App.Services;
using App.Extensions;
using App.Contracts;
using App.Conflicts;
using Audit = App.Support.AuditLog;
using Repo = App.Services.UserService;
using static App.Support.MathUtil;

namespace App.Controllers;

public class AuthController : App.Controllers.BaseController, IAuthController {
    private readonly UserService users;
    private readonly App.Services.UserService backupUsers;
    private readonly Repo repoUsers;
    private readonly IUserDirectory directory;

    public AuthController(UserService users, App.Services.UserService backupUsers, Repo repoUsers, IUserDirectory directory) {
        this.users = users;
        this.backupUsers = backupUsers;
        this.repoUsers = repoUsers;
        this.directory = directory;
    }

    public async Task<string> Login(string id) {
        var createdUsers = new UserService();
        var createdBackupUsers = new App.Services.UserService();
        UserService targetUsers = new();
        UserService? maybeUsers = users;
        UserService[] servicePool = new[] { users };
        List<UserService> listUsers = new() { users };
        Dictionary<string, UserService> usersById = new() { [id] = users };
        Lazy<UserService> lazyUsers = new(() => users);
        var inferredLazyUsers = new Lazy<UserService>(() => users);
        Task<UserService> taskUsers = Task.FromResult(users);
        ValueTask<UserService> valueTaskUsers = new(users);
        var inferredTaskUsers = new Task<UserService>(() => users);
        var inferredValueTaskUsers = new ValueTask<UserService>(users);
        List<Dictionary<string, UserService>> nestedUsers = new();
        Task<List<UserService>> taskListUsers = Task.FromResult(listUsers);
        Lazy<Dictionary<string, UserService>> lazyMappedUsers = new(() => usersById);
        var inferredTaskListUsers = new Task<List<UserService>>(() => listUsers);
        var inferredLazyMappedUsers = new Lazy<Dictionary<string, UserService>>(() => usersById);
        var optionalUsers = users?.Find(id);
        var forcedUsers = users!.Find(id);
        var optionalThisUsers = this.users?.Find(id);
        var forcedThisUsers = this.users!.Find(id);
        var asyncUsers = await users.FindAsync(id);
        var asyncThisUsers = await this.users.FindAsync(id);
        var genericUsers = users.FindAs<string>(id);
        var genericThisUsers = this.users.FindAs<string>(id);
        var maybeUser = maybeUsers.Find(id);
        var pooledUser = servicePool[0].Find(id);
        var listedUser = listUsers[0].Find(id);
        var mappedUser = usersById[id].Find(id);
        var lazyUser = lazyUsers.Value.Find(id);
        var inferredLazyUser = inferredLazyUsers.Value.Find(id);
        var taskUser = taskUsers.Result.Find(id);
        var valueTaskUser = valueTaskUsers.Result.Find(id);
        var inferredTaskUser = inferredTaskUsers.Result.Find(id);
        var inferredValueTaskUser = inferredValueTaskUsers.Result.Find(id);
        var nestedListUser = taskListUsers.Result[0].Find(id);
        var nestedMappedUser = lazyMappedUsers.Value[id].Find(id);
        var inferredNestedListUser = inferredTaskListUsers.Result[0].Find(id);
        var inferredNestedMappedUser = inferredLazyMappedUsers.Value[id].Find(id);
        var detailedUser = users.Find(id, includeDisabled: true);
        var numericUser = users.Find(42);
        var scopedUser = users.Find(id, "active");
        var namedScopedUser = users.Find(id, includeDisabled: true, scope: "admin");
        var genericDetailedUser = users.FindAs<int>(id, includeDisabled: true);
        var directoryUser = directory.Find(id);
        var directoryImplementationProfile = directory.ImplementationProfile.Load(id);
        var thisProfile = this.users.Profile.Load(id);
        var externalProfile = users.ExternalProfile.Load(id);
        var qualifiedExternalProfile = users.QualifiedExternalProfile.Load(id);
        var backupExternalProfile = backupUsers.ExternalProfile.Load(id);
        var backupQualifiedExternalProfile = backupUsers.QualifiedExternalProfile.Load(id);
        var repoExternalProfile = repoUsers.ExternalProfile.Load(id);
        var thisRepoExternalProfile = this.repoUsers.ExternalProfile.Load(id);
        var createdExternalProfile = createdUsers.ExternalProfile.Load(id);
        var createdBackupExternalProfile = createdBackupUsers.ExternalProfile.Load(id);
        var targetExternalProfile = targetUsers.ExternalProfile.Load(id);
        var lazyExternalProfile = lazyUsers.Value.ExternalProfile.Load(id);
        var inferredLazyExternalProfile = inferredLazyUsers.Value.ExternalProfile.Load(id);
        var taskExternalProfile = taskUsers.Result.ExternalProfile.Load(id);
        var valueTaskExternalProfile = valueTaskUsers.Result.ExternalProfile.Load(id);
        var inferredTaskExternalProfile = inferredTaskUsers.Result.ExternalProfile.Load(id);
        var inferredValueTaskExternalProfile = inferredValueTaskUsers.Result.ExternalProfile.Load(id);
        var optionalExternalProfile = maybeUsers?.ExternalProfile.Load(id);
        var forcedExternalProfile = maybeUsers!.ExternalProfile.Load(id);
        var pooledExternalProfile = servicePool[0].ExternalProfile.Load(id);
        var listedExternalProfile = listUsers[0].ExternalProfile.Load(id);
        var mappedExternalProfile = usersById[id].ExternalProfile.Load(id);
        var nestedListExternalProfile = taskListUsers.Result[0].ExternalProfile.Load(id);
        var nestedMappedExternalProfile = lazyMappedUsers.Value[id].ExternalProfile.Load(id);
        var inferredNestedListExternalProfile = inferredTaskListUsers.Result[0].ExternalProfile.Load(id);
        var inferredNestedMappedExternalProfile = inferredLazyMappedUsers.Value[id].ExternalProfile.Load(id);
        var thisExternalProfile = this.users.ExternalProfile.Load(id);
        var thisBackupExternalProfile = this.backupUsers.ExternalProfile.Load(id);
        var profileMetadata = users.Profile.Metadata.Display(id);
        var extensionUser = users.FormatForDisplay(id);
        var thisExtensionUser = this.users.FormatForDisplay(id);
        var optionalExtensionUser = maybeUsers?.FormatForDisplay(id);
        var forcedExtensionUser = maybeUsers!.FormatForDisplay(id);
        var listedExtensionUser = listUsers[0].FormatForDisplay(id);
        var localTag = LocalTag(id);
        var thisBaseTag = this.BaseTag(id);
        var rootTag = base.RootTag(id);
        var explicitName = App.Support.MathUtil.ClampName(id);
        var temporaryUser = new UserService().Find(id);
        var temporaryExternalProfile = new App.Services.UserService().ExternalProfile.Load(id);
        var initializedTemporaryUser = new UserService { }.Find(id);
        var initializedTemporaryProfile = new UserService { }.ExternalProfile.Load(id);
        var initializedTemporaryExternalProfile = new App.Services.UserService { }.ExternalProfile.Load(id);
        var constructedInitializedTemporaryUser = new UserService() { }.Find(id);
        var listedTemporaryUser = new List<UserService> { users }[0].Find(id);
        var mappedTemporaryUser = new Dictionary<string, UserService> { [id] = users }[id].Find(id);
        var listedTemporaryExternalProfile = new List<UserService> { users }[0].ExternalProfile.Load(id);
        var lazyTemporaryUser = new Lazy<UserService>(() => users).Value.Find(id);
        var taskTemporaryUser = new Task<UserService>(() => users).Result.Find(id);
        var lazyTemporaryExternalProfile = new Lazy<UserService>(() => users).Value.ExternalProfile.Load(id);
        var valueTaskTemporaryUser = new ValueTask<UserService>(users).Result.Find(id);
        var qualifiedLazyTemporaryUser = new System.Lazy<App.Services.UserService>(() => users).Value.Find(id);
        var qualifiedLazyTemporaryExternalProfile = new System.Lazy<App.Services.UserService>(() => users).Value.ExternalProfile.Load(id);
        App.Support.AuditLog.Record(id);
        Audit.Record(id);
        return LocalFormatter.Normalize(explicitName + temporaryUser + temporaryExternalProfile + initializedTemporaryUser + initializedTemporaryProfile + initializedTemporaryExternalProfile + constructedInitializedTemporaryUser + listedTemporaryUser + mappedTemporaryUser + listedTemporaryExternalProfile + lazyTemporaryUser + taskTemporaryUser + lazyTemporaryExternalProfile + valueTaskTemporaryUser + qualifiedLazyTemporaryUser + qualifiedLazyTemporaryExternalProfile + ClampName(rootTag + thisBaseTag + localTag + listedExtensionUser + forcedExtensionUser + optionalExtensionUser + thisExtensionUser + extensionUser + profileMetadata + inferredNestedMappedExternalProfile + inferredNestedListExternalProfile + nestedMappedExternalProfile + nestedListExternalProfile + mappedExternalProfile + listedExternalProfile + pooledExternalProfile + inferredValueTaskExternalProfile + inferredTaskExternalProfile + valueTaskExternalProfile + taskExternalProfile + forcedExternalProfile + optionalExternalProfile + inferredLazyExternalProfile + lazyExternalProfile + targetExternalProfile + createdBackupExternalProfile + createdExternalProfile + thisBackupExternalProfile + thisExternalProfile + thisRepoExternalProfile + repoExternalProfile + backupQualifiedExternalProfile + backupExternalProfile + qualifiedExternalProfile + externalProfile + thisProfile + directoryImplementationProfile + directoryUser + genericDetailedUser + namedScopedUser + scopedUser + numericUser + detailedUser + inferredNestedMappedUser + inferredNestedListUser + nestedMappedUser + nestedListUser + inferredValueTaskUser + inferredTaskUser + valueTaskUser + taskUser + inferredLazyUser + lazyUser + mappedUser + listedUser + pooledUser + maybeUser + genericUsers + genericThisUsers + asyncUsers + asyncThisUsers + optionalUsers + forcedUsers + optionalThisUsers + forcedThisUsers + users.Find(id) + this.users.Find(id) + backupUsers.Find(id) + repoUsers.Find(id) + this.repoUsers.Find(id) + createdUsers.Find(id) + createdBackupUsers.Find(id) + targetUsers.Find(id) + this.LocalTag(id) + base.BaseTag(id) + users.Profile.Load(id)));
    }

    private string LocalTag(string id) {
        return id;
    }
}

"#,
    );
    write_file(
        &dir,
        "src/App/Contracts/IAuthController.cs",
        r#"
namespace App.Contracts;

public interface IAuthController {}
"#,
    );
    write_file(
        &dir,
        "src/App/Contracts/IUserDirectory.cs",
        r#"
namespace App.Contracts;

public interface IUserDirectory {
    string Find(string id);
}
"#,
    );
    write_file(
        &dir,
        "src/App/Implementations/UserDirectory.cs",
        r#"
using App.Contracts;
using App.Profiles;

namespace App.Implementations;

public class UserDirectory : IUserDirectory {
    public ExternalProfile ImplementationProfile { get; } = new();

    public string Find(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/BaseController.cs",
        r#"
namespace App.Controllers;

public class BaseController : RootController {
    protected string BaseTag(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/RootController.cs",
        r#"
namespace App.Controllers;

public class RootController {
    protected string RootTag(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return users.Find(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
using App.Profiles;

namespace App.Services;

public class UserService {
    public ProfileService Profile { get; } = new();
    public ExternalProfile ExternalProfile { get; } = new();
    public App.Profiles.ExternalProfile QualifiedExternalProfile { get; } = new();

    public string Find(string id) {
        return id;
    }

    public string Find(string id, bool includeDisabled) {
        return id;
    }

    public string Find(int id) {
        return id.ToString();
    }

    public string Find(string id, string status) {
        return id;
    }

    public string Find(string id, bool includeDisabled, string scope) {
        return id;
    }

    public Task<string> FindAsync(string id) {
        return Task.FromResult(id);
    }

    public T FindAs<T>(string id) {
        return default!;
    }

    public T FindAs<T>(string id, bool includeDisabled) {
        return default!;
    }
}

public class ProfileService {
    public ProfileMetadata Metadata { get; } = new();

    public string Load(string id) {
        return id;
    }
}

public class ProfileMetadata {
    public string Display(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Profiles/ExternalProfile.cs",
        r#"
namespace App.Profiles;

public class ExternalProfile {
    public string Load(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Support/AuditLog.cs",
        r#"
namespace App.Support;

public static class AuditLog {
    public static void Record(string id) {}
}
"#,
    );
    write_file(
        &dir,
        "src/App/Support/MathUtil.cs",
        r#"
namespace App.Support;

public static class MathUtil {
    public static string ClampName(string name) {
        return name;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/LocalFormatter.cs",
        r#"
namespace App.Controllers;

public static class LocalFormatter {
    public static string Normalize(string name) {
        return name.Trim();
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/Audit.cs",
        r#"
namespace App.Controllers;

public static class Audit {
    public static void Record(string id) {}
}
"#,
    );
    write_file(
        &dir,
        "src/App/Conflicts/A.cs",
        r#"
namespace App.Conflicts;

public static class A {
    public static string ClampName(string name) {
        return name;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Conflicts/LocalFormatter.cs",
        r#"
namespace App.Conflicts;

public static class LocalFormatter {
    public static string Normalize(string name) {
        return name;
    }
}
"#,
    );
    dir
}

fn csharp_unqualified_base_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/AuthController.cs",
        r#"
namespace App.Controllers;

public class AuthController : BaseController {
    public string Login(string id) {
        return base.BaseTag(id) + base.RootTag(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/BaseController.cs",
        r#"
namespace App.Controllers;

public class BaseController : RootController {
    protected string BaseTag(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/RootController.cs",
        r#"
namespace App.Controllers;

public class RootController {
    protected string RootTag(string id) {
        return id;
    }
}
"#,
    );
    dir
}

fn csharp_nested_temporary_wrapper_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/AuthController.cs",
        r#"
using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using App.Services;

namespace App.Controllers;

public class AuthController {
    public string Login(string id, Dictionary<string, UserService> usersById, List<UserService> listUsers) {
        var nestedMappedUser = new Lazy<Dictionary<string, UserService>>(() => usersById).Value[id].Find(id);
        var nestedListUser = new Task<List<UserService>>(() => listUsers).Result[0].Find(id);
        var nestedMappedExternalProfile = new Lazy<Dictionary<string, UserService>>(() => usersById).Value[id].ExternalProfile.Load(id);
        return nestedMappedUser + nestedListUser + nestedMappedExternalProfile;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
using App.Profiles;

namespace App.Services;

public class UserService {
    public ExternalProfile ExternalProfile { get; } = new();

    public string Find(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Profiles/ExternalProfile.cs",
        r#"
namespace App.Profiles;

public class ExternalProfile {
    public string Load(string id) {
        return id;
    }
}
"#,
    );
    dir
}

fn csharp_extension_method_boundary_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/MissingImportController.cs",
        r#"
using App.Services;

namespace App.Controllers;

public class MissingImportController {
    public string Login(UserService users, string id) {
        return users.FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/WrongReceiverController.cs",
        r#"
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class WrongReceiverController {
    public string Login(ProductService product, string id) {
        return product.FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/MissingImportTemporaryController.cs",
        r#"
using App.Services;

namespace App.Controllers;

public class MissingImportTemporaryController {
    public string Login(string id) {
        return new UserService().FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/WrongTemporaryReceiverController.cs",
        r#"
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class WrongTemporaryReceiverController {
    public string Login(string id) {
        return new ProductService().FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/NestedTemporaryReceiverController.cs",
        r#"
using System;
using System.Collections.Generic;
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class NestedTemporaryReceiverController {
    public string Login(string id, Dictionary<string, ProductService> productsById) {
        return new Lazy<Dictionary<string, ProductService>>(() => productsById).Value[id].FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/MissingImportQualifiedTemporaryController.cs",
        r#"
namespace App.Controllers;

public class MissingImportQualifiedTemporaryController {
    public string Login(string id) {
        return new App.Services.UserService().FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Controllers/WrongQualifiedTemporaryReceiverController.cs",
        r#"
using App.Extensions;

namespace App.Controllers;

public class WrongQualifiedTemporaryReceiverController {
    public string Login(string id) {
        return new App.Services.ProductService().FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/ProductService.cs",
        r#"
namespace App.Services;

public class ProductService {}
"#,
    );
    dir
}

fn csharp_static_using_extension_conflict_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/ConflictController.cs",
        r#"
using App.Extensions;
using App.Services;
using static App.Support.DisplayFormatters;

namespace App.Controllers;

public class ConflictController {
    public string Login(UserService users, string id) {
        return FormatForDisplay(id) + users.FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Support/DisplayFormatters.cs",
        r#"
namespace App.Support;

public static class DisplayFormatters {
    public static string FormatForDisplay(string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    dir
}

fn csharp_extension_method_receiver_variant_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/ReceiverController.cs",
        r#"
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class ReceiverController {
    private readonly UserService users;

    public ReceiverController(UserService users) {
        this.users = users;
    }

    public string Login(string id) {
        UserService? maybeUsers = users;
        var optionalUser = maybeUsers?.FormatForDisplay(id);
        var forcedUser = maybeUsers!.FormatForDisplay(id);
        return this.users.FormatForDisplay(id)
            + users.FormatForDisplay(id)
            + optionalUser
            + forcedUser;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    dir
}

fn csharp_extension_method_collection_receiver_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/CollectionReceiverController.cs",
        r#"
using System.Collections.Generic;
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class CollectionReceiverController {
    public string Login(string id, UserService[] servicePool, List<UserService> listUsers, Dictionary<string, UserService> usersById) {
        return servicePool[0].FormatForDisplay(id)
            + listUsers[0].FormatForDisplay(id)
            + usersById[id].FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    dir
}

fn csharp_extension_method_wrapper_receiver_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/WrapperReceiverController.cs",
        r#"
using System;
using System.Threading.Tasks;
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class WrapperReceiverController {
    public string Login(string id, Lazy<UserService> lazyUsers, Task<UserService> taskUsers, ValueTask<UserService> valueTaskUsers) {
        return lazyUsers.Value.FormatForDisplay(id)
            + taskUsers.Result.FormatForDisplay(id)
            + valueTaskUsers.Result.FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    dir
}

fn csharp_extension_method_temporary_receiver_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/App/Controllers/TemporaryReceiverController.cs",
        r#"
using System;
using System.Threading.Tasks;
using App.Extensions;
using App.Services;

namespace App.Controllers;

public class TemporaryReceiverController {
    public string Login(string id, UserService users) {
        return new UserService().FormatForDisplay(id)
            + new UserService { }.FormatForDisplay(id)
            + new Lazy<UserService>(() => users).Value.FormatForDisplay(id)
            + new ValueTask<UserService>(users).Result.FormatForDisplay(id)
            + new App.Services.UserService().FormatForDisplay(id)
            + new System.Lazy<App.Services.UserService>(() => users).Value.FormatForDisplay(id);
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Extensions/UserServiceExtensions.cs",
        r#"
using App.Services;

namespace App.Extensions;

public static class UserServiceExtensions {
    public static string FormatForDisplay(this UserService users, string id) {
        return id;
    }
}
"#,
    );
    write_file(
        &dir,
        "src/App/Services/UserService.cs",
        r#"
namespace App.Services;

public class UserService {}
"#,
    );
    dir
}

fn rust_use_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "src/lib.rs",
        r#"
mod controllers;
mod plain;
mod repository;
mod store;
mod support;

use crate::support::audit;
use serde::Serialize;

pub fn run() {
    audit::record("root");
}
"#,
    );
    write_file(
        &dir,
        "src/repository.rs",
        r#"
pub trait Repository {
    fn load(&self, id: &str) -> String;
}
"#,
    );
    write_file(
        &dir,
        "src/store.rs",
        r#"
use crate::repository::Repository;

pub struct Store;

impl Repository for Store {
    fn load(&self, id: &str) -> String {
        id.to_string()
    }
}
"#,
    );
    write_file(
        &dir,
        "src/plain.rs",
        r#"
pub fn direct() -> &'static str {
    "direct"
}
"#,
    );
    write_file(
        &dir,
        "src/plain/mod.rs",
        r#"
pub fn nested() -> &'static str {
    "nested"
}
"#,
    );
    write_file(
        &dir,
        "src/support/mod.rs",
        r#"
pub mod audit;
pub mod nested;

use self::nested::tool;

pub fn run_nested() -> String {
    tool()
}
"#,
    );
    write_file(
        &dir,
        "src/support/nested.rs",
        r#"
pub fn tool() -> String {
    "nested".to_string()
}
"#,
    );
    write_file(
        &dir,
        "src/support/audit.rs",
        r#"
pub fn record(id: &str) {
    let _ = id;
}
"#,
    );
    write_file(
        &dir,
        "src/controllers/mod.rs",
        r#"
pub mod auth;
pub mod support;
"#,
    );
    write_file(
        &dir,
        "src/controllers/auth.rs",
        r#"
use super::support::helper;

pub fn login(id: &str) -> String {
    helper(id)
}
"#,
    );
    write_file(
        &dir,
        "src/controllers/support.rs",
        r#"
pub fn helper(id: &str) -> String {
    id.to_string()
}
"#,
    );
    dir
}

fn python_relative_import_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "app/controllers/auth.py",
        r#"
from . import support
from .support import audit
from ..core import service
import app.shared as shared
from app.shared import (
    ping as shared_ping,
    tools as shared_tools,
)
import requests


class AuthController:
    def login(self, user_id):
        audit.record(user_id)
        support.describe()
        shared.ping()
        shared_ping()
        shared_tools.pong()
        return service.load(user_id)
"#,
    );
    write_file(
        &dir,
        "app/controllers/support/__init__.py",
        r#"
def describe():
    return "support"
"#,
    );
    write_file(
        &dir,
        "app/controllers/support/audit.py",
        r#"
def record(user_id):
    return user_id
"#,
    );
    write_file(&dir, "app/core/__init__.py", "");
    write_file(
        &dir,
        "app/core/service.py",
        r#"
def load(user_id):
    return user_id
"#,
    );
    write_file(
        &dir,
        "app/shared/__init__.py",
        r#"
def ping():
    return "pong"
"#,
    );
    write_file(
        &dir,
        "app/shared/tools.py",
        r#"
def pong():
    return "ping"
"#,
    );
    dir
}

fn null_package_imports_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "package.json",
        r##"
{
  "imports": {
    "#enabled": "./src/enabled-import.ts",
    "#array": [
      null,
      "external-import-lib",
      "./src/array-import.ts"
    ],
    "#conditional": {
      "import": null,
      "default": "./src/default-import.ts"
    },
    "#conditional-external": {
      "import": "external-import-lib",
      "default": "./src/default-external-import.ts"
    }
  }
}
"##,
    );
    write_file(
        &dir,
        "tsconfig.json",
        r##"
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "#conditional": ["src/tsconfig-fallback.ts"],
      "#conditional-external": ["src/tsconfig-external-fallback.ts"]
    }
  }
}
"##,
    );
    write_file(
        &dir,
        "src/main.ts",
        r##"
import { enabledImportRender } from "#enabled";
import { arrayImportRender } from "#array";
import { conditionalImportRender } from "#conditional";
import { conditionalExternalImportRender } from "#conditional-external";

export function nullImportMain() {
  enabledImportRender();
  arrayImportRender();
  conditionalImportRender();
  conditionalExternalImportRender();
}
"##,
    );
    write_file(
        &dir,
        "src/enabled-import.ts",
        r#"
export function enabledImportRender() {
  return "enabled-import";
}
"#,
    );
    write_file(
        &dir,
        "src/array-import.ts",
        r#"
export function arrayImportRender() {
  return "array-import";
}
"#,
    );
    write_file(
        &dir,
        "external-import-lib.ts",
        r#"
export function arrayImportRender() {
  return "external-import";
}

export function conditionalExternalImportRender() {
  return "external-conditional-import";
}
"#,
    );
    write_file(
        &dir,
        "src/tsconfig-fallback.ts",
        r#"
export function conditionalImportRender() {
  return "tsconfig-fallback";
}
"#,
    );
    write_file(
        &dir,
        "src/default-import.ts",
        r#"
export function conditionalImportRender() {
  return "default-import";
}
"#,
    );
    write_file(
        &dir,
        "src/default-external-import.ts",
        r#"
export function conditionalExternalImportRender() {
  return "default-external-import";
}
"#,
    );
    write_file(
        &dir,
        "src/tsconfig-external-fallback.ts",
        r#"
export function conditionalExternalImportRender() {
  return "tsconfig-external-fallback";
}
"#,
    );
    dir
}

fn yarn_workspace_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "package.json",
        r#"
{
  "private": true,
  "workspaces": {
    "packages": ["packages/*", "!packages/legacy-*"]
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "yarn-ui": "workspace:*",
    "yarn-caret-ui": "workspace:^",
    "yarn-tilde-ui": "workspace:~",
    "yarn-version-ui": "workspace:1.2.3",
    "yarn-legacy-ui": "workspace:*"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { yarnButton } from "yarn-ui/button";
import { yarnCaretButton } from "yarn-caret-ui/button";
import { yarnTildeButton } from "yarn-tilde-ui/button";
import { yarnVersionButton } from "yarn-version-ui/button";
import { yarnLegacyButton } from "yarn-legacy-ui/button";

export function yarnWorkspaceMain() {
  yarnButton();
  yarnCaretButton();
  yarnTildeButton();
  yarnVersionButton();
  yarnLegacyButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-ui/package.json",
        r#"
{
  "name": "yarn-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-ui/src/button.ts",
        r#"
export function yarnButton() {
  return "yarn";
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-caret-ui/package.json",
        r#"
{
  "name": "yarn-caret-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-caret-ui/src/button.ts",
        r#"
export function yarnCaretButton() {
  return "yarn-caret";
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-tilde-ui/package.json",
        r#"
{
  "name": "yarn-tilde-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-tilde-ui/src/button.ts",
        r#"
export function yarnTildeButton() {
  return "yarn-tilde";
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-version-ui/package.json",
        r#"
{
  "name": "yarn-version-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/yarn-version-ui/src/button.ts",
        r#"
export function yarnVersionButton() {
  return "yarn-version";
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy-yarn-ui/package.json",
        r#"
{
  "name": "yarn-legacy-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy-yarn-ui/src/button.ts",
        r#"
export function yarnLegacyButton() {
  return "yarn-legacy-workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/yarn-legacy-ui/package.json",
        r#"
{
  "name": "yarn-legacy-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/yarn-legacy-ui/dist/button.js",
        r#"
export function yarnLegacyButton() {
  return "yarn-legacy-node";
}
"#,
    );
    dir
}

fn package_workspace_array_exclusion_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "package.json",
        r#"
{
  "private": true,
  "workspaces": ["packages/*", "!packages/legacy-*"]
}
"#,
    );
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "array-ui": "workspace:*",
    "array-legacy-ui": "workspace:*"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { arrayButton } from "array-ui/button";
import { arrayLegacyButton } from "array-legacy-ui/button";

export function arrayWorkspaceMain() {
  arrayButton();
  arrayLegacyButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/array-ui/package.json",
        r#"
{
  "name": "array-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/array-ui/src/button.ts",
        r#"
export function arrayButton() {
  return "array";
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy-array-ui/package.json",
        r#"
{
  "name": "array-legacy-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy-array-ui/src/button.ts",
        r#"
export function arrayLegacyButton() {
  return "array-legacy-workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/array-legacy-ui/package.json",
        r#"
{
  "name": "array-legacy-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/array-legacy-ui/dist/button.js",
        r#"
export function arrayLegacyButton() {
  return "array-legacy-node";
}
"#,
    );
    dir
}

fn package_workspace_array_recursive_exclusion_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "package.json",
        r#"
{
  "private": true,
  "workspaces": ["packages/**", "!packages/legacy/**"]
}
"#,
    );
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "deep-array-ui": "workspace:*",
    "deep-legacy-array-ui": "workspace:*"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { deepArrayButton } from "deep-array-ui/button";
import { deepLegacyArrayButton } from "deep-legacy-array-ui/button";

export function recursiveArrayWorkspaceMain() {
  deepArrayButton();
  deepLegacyArrayButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-array-ui/package.json",
        r#"
{
  "name": "deep-array-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-array-ui/src/button.ts",
        r#"
export function deepArrayButton() {
  return "deep-array";
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/deep-legacy-array-ui/package.json",
        r#"
{
  "name": "deep-legacy-array-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/deep-legacy-array-ui/src/button.ts",
        r#"
export function deepLegacyArrayButton() {
  return "deep-legacy-array-workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/deep-legacy-array-ui/package.json",
        r#"
{
  "name": "deep-legacy-array-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/deep-legacy-array-ui/dist/button.js",
        r#"
export function deepLegacyArrayButton() {
  return "deep-legacy-array-node";
}
"#,
    );
    dir
}

fn yarn_workspace_object_recursive_exclusion_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        "package.json",
        r#"
{
  "private": true,
  "workspaces": {
    "packages": ["packages/**", "!packages/legacy/**"]
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/package.json",
        r#"
{
  "name": "web",
  "dependencies": {
    "deep-yarn-ui": "workspace:*",
    "deep-legacy-yarn-ui": "workspace:*"
  }
}
"#,
    );
    write_file(
        &dir,
        "apps/web/src/main.ts",
        r#"
import { deepYarnButton } from "deep-yarn-ui/button";
import { deepLegacyYarnButton } from "deep-legacy-yarn-ui/button";

export function recursiveYarnWorkspaceMain() {
  deepYarnButton();
  deepLegacyYarnButton();
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-yarn-ui/package.json",
        r#"
{
  "name": "deep-yarn-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/nested/deep-yarn-ui/src/button.ts",
        r#"
export function deepYarnButton() {
  return "deep-yarn";
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/deep-legacy-yarn-ui/package.json",
        r#"
{
  "name": "deep-legacy-yarn-ui",
  "exports": {
    "./button": "./src/button.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "packages/legacy/deep-legacy-yarn-ui/src/button.ts",
        r#"
export function deepLegacyYarnButton() {
  return "deep-legacy-yarn-workspace";
}
"#,
    );
    write_file(
        &dir,
        "node_modules/deep-legacy-yarn-ui/package.json",
        r#"
{
  "name": "deep-legacy-yarn-ui",
  "exports": {
    "./button": "./dist/button.js"
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/deep-legacy-yarn-ui/dist/button.js",
        r#"
export function deepLegacyYarnButton() {
  return "deep-legacy-yarn-node";
}
"#,
    );
    dir
}

fn package_conditions_fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write_file(
        &dir,
        ".codeinsight/config.toml",
        r#"
[javascript]
package_conditions = ["types", "import", "default"]
"#,
    );
    write_file(
        &dir,
        "src/main.ts",
        r#"
import type { TypedValue } from "typed-lib";

export type AppValue = TypedValue;
"#,
    );
    write_file(
        &dir,
        "node_modules/typed-lib/package.json",
        r#"
{
  "name": "typed-lib",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js",
      "default": "./dist/default.js"
    }
  }
}
"#,
    );
    write_file(
        &dir,
        "node_modules/typed-lib/dist/index.d.ts",
        r#"
export interface TypedValue {
  value: string;
}
"#,
    );
    write_file(
        &dir,
        "node_modules/typed-lib/dist/index.js",
        r#"
export const typedValue = { value: "runtime" };
"#,
    );
    dir
}

fn long_typescript_file() -> String {
    let mut source = String::from("\nimport { render } from \"./ui\";\n\n");
    for index in 1..=85 {
        source.push_str(&format!("const filler_{index} = {index};\n"));
    }
    source.push_str("\nexport function lateEntry() {\n  render();\n}\n");
    source
}

fn huge_typescript_file() -> String {
    let mut source = String::from("export function hugeEntry() {\n");
    for index in 1..=90 {
        source.push_str(&format!(
            "  const huge_filler_{index} = \"large context filler value {index} with enough text to exceed the tiny context budget\";\n"
        ));
    }
    source.push_str("  return huge_filler_1;\n}\n");
    source
}

fn multi_long_typescript_file() -> String {
    let mut source = String::from("export function unrelatedLarge() {\n");
    for index in 1..=70 {
        source.push_str(&format!(
            "  const unrelated_filler_{index} = \"large unrelated context filler value {index} that can consume the tiny budget\";\n"
        ));
    }
    source.push_str("  return unrelated_filler_1;\n}\n\n");
    source.push_str("export function targetLater() {\n  return \"target\";\n}\n");
    source
}

fn copy_fixture(relative_path: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    copy_dir(Path::new(relative_path), dir.path());
    dir
}

fn copy_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path);
        } else {
            std::fs::copy(source_path, destination_path).unwrap();
        }
    }
}

fn write_file(dir: &TempDir, path: &str, contents: &str) {
    let path = dir.path().join(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(contents.as_bytes()).unwrap();
}
