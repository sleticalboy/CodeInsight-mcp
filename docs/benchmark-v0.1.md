# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-19 05:49:34 UTC

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real repositories across the MVP language
set and produce stable project summaries and context packs without crashing.

Environment:

- Command: `/Users/binlee/.cargo/target/release/codeinsight`
- Profile: `smoke`
- Work directory: temporary benchmark directory
- Repository subset: `all`
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: enabled

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Entrypoints | First entrypoint | Recommended tools | First recommended tool | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 184 | 10 | 0 | 58 | 5000 | pass | 216K | 4 | `index.js` | 4 | `context_pack` | 1 | 5 | 102 | 90.9% | 875 | 6000 | 0 | lower_ranked_context_omitted | false | `index.js` |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 50 | 5000 | pass | 248K | 0 | `-` | 4 | `context_pack` | 5 | 9 | 282 | 83.5% | 2663 | 6000 | 0 | complete | false | `src/itsdangerous/serializer.py` |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 86 | 5000 | pass | 264K | 12 | `gotypes/defsuses/main.go` | 4 | `context_pack` | 5 | 7 | 129 | 96.4% | 850 | 6000 | 0 | lower_ranked_context_omitted | false | `hello/hello.go` |
| memchr | Rust | `bce7df7` | 64 | 69381 | 4046 | 110 | 0 | 916 | 10000 | pass | 2.6M | 12 | `benchmarks/engines/libc/main.rs` | 4 | `context_pack` | 7 | 7 | 196 | 99.7% | 1899 | 6000 | 0 | complete | false | `src/lib.rs` |

## Key Results

- Repositories benchmarked: 4 (`all` subset).
- Agent routing: `context_pack` was the first recommended tool for 4/4 repositories.
- Context compression: selected 709 of 75753 source lines (99.1% reduction) across 18 files and 28 ranges.
- Token budget: 6287 estimated tokens total, 1572 average tokens per repository, with a 6000 token budget per context pack.
- Indexing: 1110 ms total, 278 ms average per repository, with 0 budget failures.
- Guardrails: 0 context, 0 symbol, 0 call target, and 0 call edge failures.
- Truncation: 0 context packs reported truncated output.

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 184
- Duration: 58 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 4
- First entrypoint candidate: `index.js`
- Recommended next tools: 4
- Context seed file: `index.js`
- Context task: understand limit scheduling behavior
- Context files: 1
- Context ranges: 5
- Context lines: 102 of 1123 (90.9% reduction)
- Context estimated tokens: 875
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 1
- Context truncation reason: candidate_selection_omitted_lower_ranked_context
- Context continuation status: lower_ranked_context_omitted
- Context continuation next action: narrow_task_or_seed
- First omitted candidate: none
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `index.js` | `run` | source | 0.8 | entry-like symbol named run |
| `scripts/benchmarker.js` | `run` | source | 0.8 | entry-like symbol named run |
| `index.d.ts` | `-` | source | 0.73 | conventional index file |
| `index.test-d.ts` | `-` | source | 0.73 | conventional index file |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from the highest-confidence source entrypoint. |
| `dependency_graph` | 30 | Inspect dependency edges touching the source entrypoint index.js before deeper navigation. |
| `impact_analysis` | 40 | Estimate the entrypoint change radius using call and dependency signals. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `index.js` | 5 | 1-2 | high |

Context reading plan:

| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| `index.js` | 1 | Start with seed file context and primary symbols. | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: index.js; matched task keywords: limit | Selected for high relevance via seed_file: Seed file header and imports for task: index.js; matched task keywords: limit |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 4 | 954 |
| typescript | 2 | 169 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 1 | 1 | pass |
| `selected_ranges` | >= 3 | 5 | pass |
| `reading_plan_steps` | >= 1 | 1 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_focus` | present | Start with seed file context and primary symbols. | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: index.js; matched task keywords: limit | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: index.js; matched task keywords: limit | pass |
| `estimated_tokens` | <= 1200 and applied budget | 875 / 6000 | pass |
| `line_reduction` | >= 80% | 90.9% | pass |

## itsdangerous

- URL: https://github.com/pallets/itsdangerous.git
- Commit: `672971d66a2ef9f85151e53283113f33d642dabd`
- Indexed files: 15
- Symbols: 144
- Duration: 50 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 0
- First entrypoint candidate: `-`
- Recommended next tools: 4
- Context seed file: `src/itsdangerous/serializer.py`
- Context task: understand serializer signing behavior
- Context files: 5
- Context ranges: 9
- Context lines: 282 of 1712 (83.5% reduction)
- Context estimated tokens: 2663
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context continuation next action: read_selected_context
- First omitted candidate: none
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| - | - | - | 0 | none |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from indexed source files because no source entrypoint was detected. |
| `dependency_graph` | 30 | Inspect module and package relationships; the most frequent external target is typing. |
| `callers` | 40 | Inspect static call graph edges because no source entrypoint was detected. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/itsdangerous/serializer.py` | 5 | 1-12 | high |
| `src/itsdangerous/encoding.py` | 1 | 1-40 | medium |
| `src/itsdangerous/exc.py` | 1 | 1-40 | medium |
| `src/itsdangerous/signer.py` | 1 | 1-40 | medium |
| `tests/test_itsdangerous/test_serializer.py` | 1 | 1-40 | medium |

Context reading plan:

| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| `src/itsdangerous/serializer.py` | 1 | Start with seed file authentication and session boundaries. | Where are authentication decisions, credentials, or session boundaries handled here? | `inspect_seed_file` | `file_outline` | Read this step to answer: Where are authentication decisions, credentials, or session boundaries handled here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer, signing | Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer, signing |
| `src/itsdangerous/encoding.py` | 2 | Check local dependencies that affect authentication or session boundaries. | What imported local dependency behavior affects authentication or session boundaries here? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior affects authentication or session boundaries here? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .encoding; Local dependency of src/itsdangerous/serializer.py via .encoding.want_bytes | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .encoding; Local dependency of src/itsdangerous/serializer.py via .encoding.want_bytes |
| `src/itsdangerous/exc.py` | 3 | Check local dependencies that affect authentication or session boundaries. | What imported local dependency behavior affects authentication or session boundaries here? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior affects authentication or session boundaries here? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .exc; Local dependency of src/itsdangerous/serializer.py via .exc.BadPayload; Local dependency of src/itsdangerous/serializer.py via .exc.BadSignature | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .exc; Local dependency of src/itsdangerous/serializer.py via .exc.BadPayload; Local dependency of src/itsdangerous/serializer.py via .exc.BadSignature |
| `src/itsdangerous/signer.py` | 4 | Check local dependencies that affect authentication or session boundaries. | What imported local dependency behavior affects authentication or session boundaries here? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior affects authentication or session boundaries here? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .signer; Local dependency of src/itsdangerous/serializer.py via .signer._make_keys_list; Local dependency of src/itsdangerous/serializer.py via .signer.Signer | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .signer; Local dependency of src/itsdangerous/serializer.py via .signer._make_keys_list; Local dependency of src/itsdangerous/serializer.py via .signer.Signer |
| `tests/test_itsdangerous/test_serializer.py` | 5 | Follow call graph evidence for authentication and session flow. | Which callers or callees carry authentication decisions, credentials, or session state through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees carry authentication decisions, credentials, or session state through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for medium relevance via call_graph: Call graph target of TestSerializer.serializer via serializer_factory | Selected for medium relevance via call_graph: Call graph target of TestSerializer.serializer via serializer_factory |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 15 | 1712 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 5 | pass |
| `selected_ranges` | >= 6 | 9 | pass |
| `reading_plan_steps` | >= 3 | 5 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_focus` | present | Start with seed file authentication and session boundaries. | pass |
| `first_reading_question` | present | Where are authentication decisions, credentials, or session boundaries handled here? | pass |
| `first_reading_reason` | present | Read this step to answer: Where are authentication decisions, credentials, or session boundaries handled here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer, signing | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer, signing | pass |
| `estimated_tokens` | <= 3000 and applied budget | 2663 / 6000 | pass |
| `line_reduction` | >= 80% | 83.5% | pass |

## go-example

- URL: https://github.com/golang/example.git
- Commit: `7f05d217867b2af52b0a28c6d1c91df97e1b5b39`
- Indexed files: 38
- Symbols: 189
- Duration: 86 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 12
- First entrypoint candidate: `gotypes/defsuses/main.go`
- Recommended next tools: 4
- Context seed file: `hello/hello.go`
- Context task: understand hello server behavior
- Context files: 5
- Context ranges: 7
- Context lines: 129 of 3537 (96.4% reduction)
- Context estimated tokens: 850
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 1
- Context truncation reason: candidate_selection_omitted_lower_ranked_context
- Context continuation status: lower_ranked_context_omitted
- Context continuation next action: narrow_task_or_seed
- First omitted candidate: none
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `gotypes/defsuses/main.go` | `main` | source | 1.0 | entry symbol named main |
| `gotypes/doc/main.go` | `main` | source | 1.0 | entry symbol named main |
| `gotypes/hello/hello.go` | `main` | source | 1.0 | entry symbol named main |
| `gotypes/hugeparam/main.go` | `main` | source | 1.0 | entry symbol named main |
| `gotypes/implements/main.go` | `main` | source | 1.0 | entry symbol named main |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from the highest-confidence source entrypoint. |
| `dependency_graph` | 30 | Inspect dependency edges touching the source entrypoint gotypes/defsuses/main.go before deeper navigation. |
| `impact_analysis` | 40 | Estimate the entrypoint change radius using call and dependency signals. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `hello/hello.go` | 3 | 1-24 | high |
| `helloserver/server.go` | 1 | 41-45 | high |
| `gotypes/defsuses/main.go` | 1 | 1-40 | medium |
| `internal/cmd/weave/weave.go` | 1 | 47-51 | medium |
| `hello/reverse/reverse.go` | 1 | 1-15 | medium |

Context reading plan:

| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| `hello/hello.go` | 1 | Start with seed file context and primary symbols. | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello |
| `helloserver/server.go` | 2 | Follow static call graph evidence around the seed flow. | Which callers or callees explain how control moves through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for high relevance via call_graph: Call graph caller of usage via main | Selected for high relevance via call_graph: Call graph caller of usage via main |
| `gotypes/defsuses/main.go` | 3 | Follow static call graph evidence around the seed flow. | Which callers or callees explain how control moves through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for medium relevance via call_graph: Call graph target of main via PrintDefsUses | Selected for medium relevance via call_graph: Call graph target of main via PrintDefsUses |
| `internal/cmd/weave/weave.go` | 4 | Follow static call graph evidence around the seed flow. | Which callers or callees explain how control moves through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for medium relevance via call_graph: Call graph caller of flag.Usage via main | Selected for medium relevance via call_graph: Call graph caller of flag.Usage via main |
| `hello/reverse/reverse.go` | 5 | Check local dependency context that supports selected files. | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of hello/hello.go via golang.org/x/example/hello/reverse | Selected for medium relevance via dependency: Local dependency of hello/hello.go via golang.org/x/example/hello/reverse |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 37 | 3523 |
| javascript | 1 | 14 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 5 | pass |
| `selected_ranges` | >= 4 | 7 | pass |
| `reading_plan_steps` | >= 3 | 5 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_focus` | present | Start with seed file context and primary symbols. | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | pass |
| `estimated_tokens` | <= 1000 and applied budget | 850 / 6000 | pass |
| `line_reduction` | >= 90% | 96.4% | pass |

## memchr

- URL: https://github.com/BurntSushi/memchr.git
- Commit: `bce7df7140acff420478a358cde5587904000cb1`
- Indexed files: 64
- Symbols: 4046
- Duration: 916 ms
- Index budget: 10000 ms (pass)
- Entrypoint candidates: 12
- First entrypoint candidate: `benchmarks/engines/libc/main.rs`
- Recommended next tools: 4
- Context seed file: `src/lib.rs`
- Context task: understand memchr finder API
- Context files: 7
- Context ranges: 7
- Context lines: 196 of 69381 (99.7% reduction)
- Context estimated tokens: 1899
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context continuation next action: read_selected_context
- First omitted candidate: none
- Context truncated: false

Entrypoint candidates:

| File | Symbol | Role | Confidence | Reason |
| --- | --- | --- | ---: | --- |
| `benchmarks/engines/libc/main.rs` | `main` | source | 1.0 | entry symbol named main |
| `benchmarks/engines/rust-bytecount/main.rs` | `main` | source | 1.0 | entry symbol named main |
| `benchmarks/engines/rust-jetscii/main.rs` | `main` | source | 1.0 | entry symbol named main |
| `benchmarks/engines/rust-memchr/main.rs` | `main` | source | 1.0 | entry symbol named main |
| `benchmarks/engines/rust-memchrold/main.rs` | `main` | source | 1.0 | entry symbol named main |

Recommended next tools:

| Tool | Priority | Reason |
| --- | ---: | --- |
| `context_pack` | 10 | Build first-read context from the highest-confidence source entrypoint. |
| `dependency_graph` | 30 | Inspect dependency edges touching the source entrypoint benchmarks/engines/libc/main.rs before deeper navigation. |
| `impact_analysis` | 40 | Estimate the entrypoint change radius using call and dependency signals. |
| `config_status` | 80 | Check project-specific validation commands before planning changes. |

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/lib.rs` | 1 | 1-1 | high |
| `src/memchr.rs` | 1 | 1-40 | medium |
| `src/cow.rs` | 1 | 1-40 | medium |
| `src/ext.rs` | 1 | 1-40 | medium |
| `src/macros.rs` | 1 | 1-20 | medium |
| `src/vector.rs` | 1 | 1-40 | medium |
| `src/tests/mod.rs` | 1 | 1-15 | medium |

Context reading plan:

| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| `src/lib.rs` | 1 | Start with seed file context and primary symbols. | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs; matched task keywords: api, finder, memchr | Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs; matched task keywords: api, finder, memchr |
| `src/memchr.rs` | 2 | Check local dependency context that supports selected files. | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via memchr | Selected for medium relevance via dependency: Local dependency of src/lib.rs via memchr |
| `src/cow.rs` | 3 | Check local dependency context that supports selected files. | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via cow | Selected for medium relevance via dependency: Local dependency of src/lib.rs via cow |
| `src/ext.rs` | 4 | Check local dependency context that supports selected files. | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via ext | Selected for medium relevance via dependency: Local dependency of src/lib.rs via ext |
| `src/macros.rs` | 5 | Check local dependency context that supports selected files. | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via macros | Selected for medium relevance via dependency: Local dependency of src/lib.rs via macros |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 64 | 69381 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 5 | 7 | pass |
| `selected_ranges` | >= 5 | 7 | pass |
| `reading_plan_steps` | >= 5 | 7 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_focus` | present | Start with seed file context and primary symbols. | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs; matched task keywords: api, finder, memchr | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs; matched task keywords: api, finder, memchr | pass |
| `estimated_tokens` | <= 2500 and applied budget | 1899 / 6000 | pass |
| `line_reduction` | >= 95% | 99.7% | pass |
