# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-15 01:02:14 UTC

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real public repositories across the MVP
language set and produce stable project summaries and context packs without
crashing.

Environment:

- Command: `/Users/binlee/.cargo/target/release/codeinsight`
- Profile: `smoke`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: enabled

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Entrypoints | First entrypoint | Recommended tools | First recommended tool | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 184 | 10 | 0 | 55 | 5000 | pass | 216K | 4 | `index.js` | 4 | `context_pack` | 1 | 5 | 102 | 90.9% | 875 | 6000 | 0 | lower_ranked_context_omitted | false | `index.js` |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 46 | 5000 | pass | 248K | 0 | `-` | 4 | `context_pack` | 4 | 8 | 242 | 85.9% | 2373 | 6000 | 0 | complete | false | `src/itsdangerous/serializer.py` |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 91 | 5000 | pass | 264K | 12 | `gotypes/defsuses/main.go` | 4 | `context_pack` | 4 | 6 | 89 | 97.5% | 600 | 6000 | 0 | lower_ranked_context_omitted | false | `hello/hello.go` |
| memchr | Rust | `bce7df7` | 64 | 69381 | 4046 | 110 | 0 | 692 | 10000 | pass | 2.3M | 12 | `benchmarks/engines/libc/main.rs` | 4 | `context_pack` | 7 | 7 | 196 | 99.7% | 1899 | 6000 | 0 | complete | false | `src/lib.rs` |

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 184
- Duration: 55 ms
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

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 4 | 954 |
| typescript | 2 | 169 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | > 0 | 1 | pass |
| `selected_ranges` | > 0 | 5 | pass |
| `reading_plan_steps` | > 0 | 1 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= applied budget | 875 / 6000 | pass |
| `line_reduction` | >= 50% | 90.9% | pass |

## itsdangerous

- URL: https://github.com/pallets/itsdangerous.git
- Commit: `672971d66a2ef9f85151e53283113f33d642dabd`
- Indexed files: 15
- Symbols: 144
- Duration: 46 ms
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

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 15 | 1712 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | > 0 | 4 | pass |
| `selected_ranges` | > 0 | 8 | pass |
| `reading_plan_steps` | > 0 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= applied budget | 2373 / 6000 | pass |
| `line_reduction` | >= 50% | 85.9% | pass |

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

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 37 | 3523 |
| javascript | 1 | 14 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | > 0 | 4 | pass |
| `selected_ranges` | > 0 | 6 | pass |
| `reading_plan_steps` | > 0 | 4 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= applied budget | 600 / 6000 | pass |
| `line_reduction` | >= 50% | 97.5% | pass |

## memchr

- URL: https://github.com/BurntSushi/memchr.git
- Commit: `bce7df7140acff420478a358cde5587904000cb1`
- Indexed files: 64
- Symbols: 4046
- Duration: 692 ms
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

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 64 | 69381 |

Context pack guardrails:

| Check | Expectation | Observed | Status |
| --- | --- | --- | --- |
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | > 0 | 7 | pass |
| `selected_ranges` | > 0 | 7 | pass |
| `reading_plan_steps` | > 0 | 7 | pass |
| `first_next_action` | present | inspect_seed_file | pass |
| `estimated_tokens` | <= applied budget | 1899 / 6000 | pass |
| `line_reduction` | >= 50% | 99.7% | pass |
