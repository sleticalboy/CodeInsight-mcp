# CodeInsight v0.1 Large Repository Benchmark

Generated at: 2026-07-05 16:13:44 UTC

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real public repositories across the MVP
language set and produce stable project summaries and context packs without
crashing.

Environment:

- Command: `target/release/codeinsight`
- Profile: `large`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | DB size | Context files | Ranges | Tokens | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| express | JavaScript | `18e5985` | 141 | 21440 | 1899 | 72 | 0 | 147099 | 3.9M | 3 | 5 | 645 | false | `lib/application.js` |
| flask | Python | `36e4a82` | 83 | 18337 | 1620 | 153 | 0 | 278 | 756K | 1 | 4 | 1834 | false | `src/flask/app.py` |
| gin | Go | `34dac20` | 99 | 24099 | 1857 | 31 | 0 | 452 | 1.3M | 1 | 9 | 2387 | false | `gin.go` |
| tokio | Rust | `c637f6e` | 789 | 177186 | 8447 | 75 | 0 | 5150 | 4.6M | 5 | 7 | 979 | false | `tokio/src/lib.rs` |

## Details

## express

- URL: https://github.com/expressjs/express.git
- Commit: `18e5985b8a9d5e8423db0a9121f22bdaecd5b120`
- Indexed files: 141
- Symbols: 1899
- Duration: 147099 ms
- Context seed file: `lib/application.js`
- Context task: understand express application routing behavior
- Context files: 3
- Context ranges: 5
- Context estimated tokens: 645
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `lib/application.js` | 3 | 1-7 | high |
| `lib/utils.js` | 1 | 1-40 | medium |
| `lib/view.js` | 1 | 1-40 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 141 | 21440 |

## flask

- URL: https://github.com/pallets/flask.git
- Commit: `36e4a824f340fdee7ed50937ba8e7f6bc7d17f81`
- Indexed files: 83
- Symbols: 1620
- Duration: 278 ms
- Context seed file: `src/flask/app.py`
- Context task: understand flask application dispatch behavior
- Context files: 1
- Context ranges: 4
- Context estimated tokens: 1834
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/flask/app.py` | 4 | 1-40 | high |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 83 | 18337 |

## gin

- URL: https://github.com/gin-gonic/gin.git
- Commit: `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd`
- Indexed files: 99
- Symbols: 1857
- Duration: 452 ms
- Context seed file: `gin.go`
- Context task: understand gin engine routing behavior
- Context files: 1
- Context ranges: 9
- Context estimated tokens: 2387
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `gin.go` | 9 | 1-7 | high |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 99 | 24099 |

## tokio

- URL: https://github.com/tokio-rs/tokio.git
- Commit: `c637f6e73d06f36d933cc3edaf45111c06b79c18`
- Indexed files: 789
- Symbols: 8447
- Duration: 5150 ms
- Context seed file: `tokio/src/lib.rs`
- Context task: understand tokio runtime public API
- Context files: 5
- Context ranges: 7
- Context estimated tokens: 979
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `tokio/src/lib.rs` | 3 | 1-1 | high |
| `tokio/src/blocking.rs` | 1 | 1-40 | medium |
| `tokio/src/future/mod.rs` | 1 | 1-28 | medium |
| `tokio/src/loom/mod.rs` | 1 | 1-14 | medium |
| `tokio/src/util/mod.rs` | 1 | 1-40 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 789 | 177186 |
