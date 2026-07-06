# CodeInsight v0.1 Large Repository Benchmark

Generated at: 2026-07-06 03:50:53 UTC

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
- Index budget mode: enabled

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Context files | Ranges | Context lines | Line reduction | Tokens | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| express | JavaScript | `66878d3` | 141 | 21440 | 2428 | 72 | 0 | 3205 | 10000 | pass | 5.6M | 4 | 13 | 362 | 98.3% | 2360 | false | `lib/application.js` |
| flask | Python | `36e4a82` | 83 | 18337 | 1620 | 153 | 0 | 292 | 5000 | pass | 968K | 1 | 4 | 159 | 99.1% | 1834 | false | `src/flask/app.py` |
| gin | Go | `34dac20` | 99 | 24099 | 1857 | 31 | 0 | 453 | 5000 | pass | 1.6M | 1 | 9 | 229 | 99.0% | 2387 | false | `gin.go` |
| tokio | Rust | `c637f6e` | 789 | 177186 | 8447 | 75 | 0 | 3049 | 20000 | pass | 5.8M | 5 | 7 | 135 | 99.9% | 979 | false | `tokio/src/lib.rs` |

## Details

## express

- URL: https://github.com/expressjs/express.git
- Commit: `66878d3e70437ba7b887ec519a3e33edc5bca0c7`
- Indexed files: 141
- Symbols: 2428
- Duration: 3205 ms
- Index budget: 10000 ms (pass)
- Context seed file: `lib/application.js`
- Context task: understand express application routing behavior
- Context files: 4
- Context ranges: 13
- Context lines: 362 of 21440 (98.3% reduction)
- Context estimated tokens: 2360
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `lib/application.js` | 10 | 1-7 | high |
| `lib/utils.js` | 1 | 1-40 | high |
| `lib/view.js` | 1 | 1-40 | high |
| `index.js` | 1 | 1-11 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| javascript | 141 | 21440 |

Symbol target guardrails:

| Target | Minimum symbols | Observed symbols | Status |
| --- | ---: | ---: | --- |
| `createError` | 1 | 14 | pass |
| `handleError` | 1 | 10 | pass |
| `User.index` | 1 | 1 | pass |
| `User.range` | 1 | 1 | pass |
| `users.list` | 1 | 1 | pass |
| `METHODS` | 1 | 1 | pass |
| `Buffer` | 1 | 13 | pass |
| `address` | 1 | 2 | pass |
| `port` | 1 | 1 | pass |

Call target guardrails:

| Target | Minimum calls | Observed calls | Status |
| --- | ---: | ---: | --- |
| `app.get` | 1 | 196 | pass |
| `app.<dynamic>` | 1 | 5 | pass |
| `app.route.get` | 1 | 2 | pass |
| `router.route.get` | 1 | 1 | pass |

Call edge guardrails:

| Caller | Callee | Minimum calls | Observed calls | Status |
| --- | --- | ---: | ---: | --- |
| `it.<callback>` | `app.route.get` | 1 | 2 | pass |
| `app.get.<callback>` | `res.send` | 1 | 52 | pass |

## flask

- URL: https://github.com/pallets/flask.git
- Commit: `36e4a824f340fdee7ed50937ba8e7f6bc7d17f81`
- Indexed files: 83
- Symbols: 1620
- Duration: 292 ms
- Index budget: 5000 ms (pass)
- Context seed file: `src/flask/app.py`
- Context task: understand flask application dispatch behavior
- Context files: 1
- Context ranges: 4
- Context lines: 159 of 18337 (99.1% reduction)
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
- Duration: 453 ms
- Index budget: 5000 ms (pass)
- Context seed file: `gin.go`
- Context task: understand gin engine routing behavior
- Context files: 1
- Context ranges: 9
- Context lines: 229 of 24099 (99.0% reduction)
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
- Duration: 3049 ms
- Index budget: 20000 ms (pass)
- Context seed file: `tokio/src/lib.rs`
- Context task: understand tokio runtime public API
- Context files: 5
- Context ranges: 7
- Context lines: 135 of 177186 (99.9% reduction)
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
