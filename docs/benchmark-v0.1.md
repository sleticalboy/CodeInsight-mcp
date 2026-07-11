# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-11 11:00:19 UTC

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real public repositories across the MVP
language set and produce stable project summaries and context packs without
crashing.

Environment:

- Command: `target/release/codeinsight`
- Profile: `smoke`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: enabled

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 184 | 10 | 0 | 69 | 5000 | pass | 216K | 1 | 5 | 102 | 90.9% | 875 | 6000 | 0 | lower_ranked_context_omitted | false | `index.js` |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 47 | 5000 | pass | 248K | 4 | 8 | 242 | 85.9% | 2373 | 6000 | 0 | complete | false | `src/itsdangerous/serializer.py` |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 79 | 5000 | pass | 264K | 4 | 6 | 89 | 97.5% | 600 | 6000 | 0 | lower_ranked_context_omitted | false | `hello/hello.go` |
| memchr | Rust | `4e1c173` | 64 | 69371 | 4045 | 110 | 0 | 1464 | 10000 | pass | 3.3M | 7 | 7 | 196 | 99.7% | 1899 | 6000 | 0 | complete | false | `src/lib.rs` |

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 184
- Duration: 69 ms
- Index budget: 5000 ms (pass)
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

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `index.js` | 5 | 1-2 | high |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 4 | 954 |
| typescript | 2 | 169 |

## itsdangerous

- URL: https://github.com/pallets/itsdangerous.git
- Commit: `672971d66a2ef9f85151e53283113f33d642dabd`
- Indexed files: 15
- Symbols: 144
- Duration: 47 ms
- Index budget: 5000 ms (pass)
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

## go-example

- URL: https://github.com/golang/example.git
- Commit: `7f05d217867b2af52b0a28c6d1c91df97e1b5b39`
- Indexed files: 38
- Symbols: 189
- Duration: 79 ms
- Index budget: 5000 ms (pass)
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

## memchr

- URL: https://github.com/BurntSushi/memchr.git
- Commit: `4e1c173d87851937d0f9683e1d1d417e521d6ca7`
- Indexed files: 64
- Symbols: 4045
- Duration: 1464 ms
- Index budget: 10000 ms (pass)
- Context seed file: `src/lib.rs`
- Context task: understand memchr finder API
- Context files: 7
- Context ranges: 7
- Context lines: 196 of 69371 (99.7% reduction)
- Context estimated tokens: 1899
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

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
| rust | 64 | 69371 |
