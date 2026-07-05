# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-05 16:28:11 UTC

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

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Context files | Ranges | Tokens | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- | --- |
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 162 | 10 | 0 | 45 | 5000 | pass | 160K | 1 | 3 | 835 | false | `index.js` |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 41 | 5000 | pass | 168K | 1 | 3 | 1332 | false | `src/itsdangerous/serializer.py` |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 75 | 5000 | pass | 204K | 1 | 3 | 414 | false | `hello/hello.go` |
| memchr | Rust | `e21e9fb` | 64 | 69365 | 4045 | 100 | 0 | 858 | 10000 | pass | 2.1M | 7 | 7 | 1899 | false | `src/lib.rs` |

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 162
- Duration: 45 ms
- Index budget: 5000 ms (pass)
- Context seed file: `index.js`
- Context task: understand limit scheduling behavior
- Context files: 1
- Context ranges: 3
- Context estimated tokens: 835
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `index.js` | 3 | 1-2 | high |

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
- Duration: 41 ms
- Index budget: 5000 ms (pass)
- Context seed file: `src/itsdangerous/serializer.py`
- Context task: understand serializer signing behavior
- Context files: 1
- Context ranges: 3
- Context estimated tokens: 1332
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/itsdangerous/serializer.py` | 3 | 1-12 | high |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 15 | 1712 |

## go-example

- URL: https://github.com/golang/example.git
- Commit: `7f05d217867b2af52b0a28c6d1c91df97e1b5b39`
- Indexed files: 38
- Symbols: 189
- Duration: 75 ms
- Index budget: 5000 ms (pass)
- Context seed file: `hello/hello.go`
- Context task: understand hello server behavior
- Context files: 1
- Context ranges: 3
- Context estimated tokens: 414
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `hello/hello.go` | 3 | 1-24 | high |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 37 | 3523 |
| javascript | 1 | 14 |

## memchr

- URL: https://github.com/BurntSushi/memchr.git
- Commit: `e21e9fb47c4362d93a24ce969b20fd778d8618c8`
- Indexed files: 64
- Symbols: 4045
- Duration: 858 ms
- Index budget: 10000 ms (pass)
- Context seed file: `src/lib.rs`
- Context task: understand memchr finder API
- Context files: 7
- Context ranges: 7
- Context estimated tokens: 1899
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/lib.rs` | 1 | 1-1 | high |
| `src/memchr.rs` | 1 | 1-40 | medium |
| `src/cow.rs` | 1 | 1-40 | medium |
| `src/ext.rs` | 1 | 1-40 | medium |
| `src/macros.rs` | 1 | 1-20 | medium |
| `src/tests/mod.rs` | 1 | 1-15 | medium |
| `src/vector.rs` | 1 | 1-40 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 64 | 69365 |
