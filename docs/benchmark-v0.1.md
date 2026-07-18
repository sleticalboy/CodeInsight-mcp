# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-18 14:34:08 UTC

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
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 184 | 10 | 0 | 60 | 5000 | pass | 216K | 4 | `index.js` | 4 | `context_pack` | 1 | 5 | 102 | 90.9% | 875 | 6000 | 0 | lower_ranked_context_omitted | false | `index.js` |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 50 | 5000 | pass | 248K | 0 | `-` | 4 | `context_pack` | 4 | 8 | 242 | 85.9% | 2373 | 6000 | 0 | complete | false | `src/itsdangerous/serializer.py` |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 91 | 5000 | pass | 264K | 12 | `gotypes/defsuses/main.go` | 4 | `context_pack` | 4 | 6 | 89 | 97.5% | 600 | 6000 | 0 | lower_ranked_context_omitted | false | `hello/hello.go` |
| memchr | Rust | `bce7df7` | 64 | 69381 | 4046 | 110 | 0 | 750 | 10000 | pass | 2.3M | 12 | `benchmarks/engines/libc/main.rs` | 4 | `context_pack` | 7 | 7 | 196 | 99.7% | 1899 | 6000 | 0 | complete | false | `src/lib.rs` |

## Key Results

- Repositories benchmarked: 4 (`all` subset).
- Agent routing: `context_pack` was the first recommended tool for 4/4 repositories.
- Context compression: selected 629 of 75753 source lines (99.2% reduction) across 16 files and 26 ranges.
- Token budget: 5747 estimated tokens total, 1437 average tokens per repository, with a 6000 token budget per context pack.
- Indexing: 951 ms total, 238 ms average per repository, with 0 budget failures.
- Guardrails: 0 context, 0 symbol, 0 call target, and 0 call edge failures.
- Truncation: 0 context packs reported truncated output.

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 184
- Duration: 60 ms
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

| File | Rank | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- |
| `index.js` | 1 | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: index.js | Selected for high relevance via seed_file: Seed file header and imports for task: index.js |

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
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: index.js | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: index.js | pass |
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
- Context files: 4
- Context ranges: 8
- Context lines: 242 of 1712 (85.9% reduction)
- Context estimated tokens: 2373
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

Context reading plan:

| File | Rank | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- |
| `src/itsdangerous/serializer.py` | 1 | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer | Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer |
| `src/itsdangerous/encoding.py` | 2 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .encoding; Local dependency of src/itsdangerous/serializer.py via .encoding.want_bytes | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .encoding; Local dependency of src/itsdangerous/serializer.py via .encoding.want_bytes |
| `src/itsdangerous/exc.py` | 3 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .exc; Local dependency of src/itsdangerous/serializer.py via .exc.BadPayload; Local dependency of src/itsdangerous/serializer.py via .exc.BadSignature | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .exc; Local dependency of src/itsdangerous/serializer.py via .exc.BadPayload; Local dependency of src/itsdangerous/serializer.py via .exc.BadSignature |
| `src/itsdangerous/signer.py` | 4 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .signer; Local dependency of src/itsdangerous/serializer.py via .signer._make_keys_list; Local dependency of src/itsdangerous/serializer.py via .signer.Signer | Selected for medium relevance via dependency: Local dependency of src/itsdangerous/serializer.py via .signer; Local dependency of src/itsdangerous/serializer.py via .signer._make_keys_list; Local dependency of src/itsdangerous/serializer.py via .signer.Signer |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 15 | 1712 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 4 | pass |
| `selected_ranges` | >= 6 | 8 | pass |
| `reading_plan_steps` | >= 3 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: src/itsdangerous/serializer.py; matched task keywords: serializer | pass |
| `estimated_tokens` | <= 3000 and applied budget | 2373 / 6000 | pass |
| `line_reduction` | >= 80% | 85.9% | pass |

## go-example

- URL: https://github.com/golang/example.git
- Commit: `7f05d217867b2af52b0a28c6d1c91df97e1b5b39`
- Indexed files: 38
- Symbols: 189
- Duration: 91 ms
- Index budget: 5000 ms (pass)
- Entrypoint candidates: 12
- First entrypoint candidate: `gotypes/defsuses/main.go`
- Recommended next tools: 4
- Context seed file: `hello/hello.go`
- Context task: understand hello server behavior
- Context files: 4
- Context ranges: 6
- Context lines: 89 of 3537 (97.5% reduction)
- Context estimated tokens: 600
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
| `internal/cmd/weave/weave.go` | 1 | 47-51 | medium |
| `hello/reverse/reverse.go` | 1 | 1-15 | medium |

Context reading plan:

| File | Rank | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- |
| `hello/hello.go` | 1 | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello |
| `helloserver/server.go` | 2 | Which callers or callees explain how control moves through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for high relevance via call_graph: Call graph caller of usage via main | Selected for high relevance via call_graph: Call graph caller of usage via main |
| `internal/cmd/weave/weave.go` | 3 | Which callers or callees explain how control moves through this flow? | `follow_call_graph` | `impact_analysis` | Read this step to answer: Which callers or callees explain how control moves through this flow? If deeper evidence is needed, call impact_analysis. Selection reason: Selected for medium relevance via call_graph: Call graph caller of flag.Usage via main | Selected for medium relevance via call_graph: Call graph caller of flag.Usage via main |
| `hello/reverse/reverse.go` | 4 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of hello/hello.go via golang.org/x/example/hello/reverse | Selected for medium relevance via dependency: Local dependency of hello/hello.go via golang.org/x/example/hello/reverse |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 37 | 3523 |
| javascript | 1 | 14 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 3 | 4 | pass |
| `selected_ranges` | >= 4 | 6 | pass |
| `reading_plan_steps` | >= 3 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: hello/hello.go; matched task keywords: hello | pass |
| `estimated_tokens` | <= 1000 and applied budget | 600 / 6000 | pass |
| `line_reduction` | >= 90% | 97.5% | pass |

## memchr

- URL: https://github.com/BurntSushi/memchr.git
- Commit: `bce7df7140acff420478a358cde5587904000cb1`
- Indexed files: 64
- Symbols: 4046
- Duration: 750 ms
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

| File | Rank | Question | Next action | Suggested tool | Reason | Selection reason |
| --- | ---: | --- | --- | --- | --- | --- |
| `src/lib.rs` | 1 | What entrypoints, exported symbols, or setup code define the main flow here? | `inspect_seed_file` | `file_outline` | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs | Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs |
| `src/memchr.rs` | 2 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via memchr | Selected for medium relevance via dependency: Local dependency of src/lib.rs via memchr |
| `src/cow.rs` | 3 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via cow | Selected for medium relevance via dependency: Local dependency of src/lib.rs via cow |
| `src/ext.rs` | 4 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via ext | Selected for medium relevance via dependency: Local dependency of src/lib.rs via ext |
| `src/macros.rs` | 5 | What imported local dependency behavior is required to understand this file? | `inspect_dependency` | `dependency_graph` | Read this step to answer: What imported local dependency behavior is required to understand this file? If deeper evidence is needed, call dependency_graph. Selection reason: Selected for medium relevance via dependency: Local dependency of src/lib.rs via macros | Selected for medium relevance via dependency: Local dependency of src/lib.rs via macros |

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
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | Selected for high relevance via seed_file: Seed file header and imports for task: src/lib.rs | pass |
| `estimated_tokens` | <= 2500 and applied budget | 1899 / 6000 | pass |
| `line_reduction` | >= 95% | 99.7% | pass |
