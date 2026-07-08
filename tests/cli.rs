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
    assert_eq!(index["indexed_files"], 17);
    assert_eq!(index["changed_files"], 17);
    assert_eq!(index["errors"].as_array().unwrap().len(), 0);

    let second_index = run_json(["index", fixture.path().to_str().unwrap()]);
    assert_eq!(second_index["changed_files"], 0);
    assert_eq!(second_index["unchanged_files"], 17);

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
    assert!(targets.contains(&"@fallback/fallback-ui"));
    assert!(targets.contains(&"shared"));
    assert!(targets.contains(&"fixture-lib/package-ui"));
    assert!(targets.contains(&"dep-lib/feature"));
    assert!(targets.contains(&"dep-lib/node-feature"));
    assert!(targets.contains(&"legacy-lib"));
    assert!(targets.contains(&"legacy-lib/plugin"));
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
                dependency["target"] == "@app/path-ui"
                    && dependency["resolved_file"] == "src/path-ui.ts"
            })
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
    assert_eq!(context["symbols"][0]["name"], "AuthService");
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
        "1600",
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
fn cli_config_status_reports_loaded_and_detected_commands() {
    let fixture = TempDir::new().unwrap();
    write_file(&fixture, "Cargo.toml", "[package]\nname = \"demo\"\n");

    let missing = run_json(["config-status", fixture.path().to_str().unwrap()]);
    assert_eq!(missing["exists"], false);
    assert_eq!(missing["loaded"], false);
    assert_eq!(missing["commands_override_builtin"], false);
    assert_eq!(missing["detected_test_commands"][0], "cargo test --locked");

    write_file(
        &fixture,
        ".codeinsight/config.toml",
        r#"
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
    assert_eq!(loaded["commands_override_builtin"], true);
    assert!(loaded.get("parse_error").is_none());
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
    assert_eq!(status["detected_test_commands"][0], "cargo test --locked");
    assert!(
        status["parse_error"]
            .as_str()
            .is_some_and(|error| error.contains(".codeinsight/config.toml"))
    );

    run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
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
    write_file(&fixture, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 3);

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
        impact["impact_counts"]["paths"].as_u64().unwrap()
            >= impact["paths"].as_array().unwrap().len() as u64
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

    let index = run_json(["index", fixture.path().to_str().unwrap(), "--force"]);
    assert_eq!(index["indexed_files"], 4);

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

fn fixture_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    write_file(
        &dir,
        "package.json",
        r#"
{
  "name": "fixture-lib",
  "exports": {
    "./package-*": "./src/package-*.ts"
  }
}
"#,
    );
    write_file(
        &dir,
        "tsconfig.json",
        r#"
{
  "compilerOptions": {
    "baseUrl": "src",
    "paths": {
      "@app/*": ["*"],
      "@fallback/*": ["missing/*", "*"]
    }
  }
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
        r#"
import { render } from "./ui";
import drawDefault from "./ui";
import { relayRender, relayDefault, render as starRender, uiApi } from "./barrel";
import { finalApi, finalDefault, finalRender } from "./barrel2";
import * as ui from "./ui";
import { pathRender } from "@app/path-ui";
import { fallbackRender } from "@fallback/fallback-ui";
import { sharedRender } from "shared";
import { packageRender } from "fixture-lib/package-ui";
import { depRender } from "dep-lib/feature";
import { depNodeRender } from "dep-lib/node-feature";
import { legacyRender } from "legacy-lib";
import { legacyPluginRender } from "legacy-lib/plugin";
const { render: draw } = require("./ui");
const uiModule = require("./ui");
const computedUiModule = require("./" + "ui");

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
  depNodeRender();
  legacyRender();
  legacyPluginRender();
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
        "src/package-ui.ts",
        r#"
export function packageRender() {
  return "package";
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
