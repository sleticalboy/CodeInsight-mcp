# CodeInsight v0.1 Large Repository Benchmark

Generated at: 2026-07-11 11:00:55 UTC

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

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
| express | JavaScript | `ba00676` | 141 | 21440 | 2428 | 72 | 0 | 5925 | 10000 | pass | 6.1M | 4 | 13 | 362 | 98.3% | 2360 | 6000 | 0 | complete | false | `lib/application.js` |
| flask | Python | `36e4a82` | 83 | 18337 | 1620 | 153 | 0 | 588 | 5000 | pass | 1.2M | 12 | 15 | 573 | 96.9% | 5851 | 6000 | 2 | omitted_candidates_available | true | `src/flask/app.py` |
| gin | Go | `34dac20` | 99 | 24099 | 1857 | 31 | 0 | 611 | 5000 | pass | 1.8M | 4 | 12 | 305 | 98.7% | 2969 | 6000 | 0 | complete | false | `gin.go` |
| tokio | Rust | `9c465e2` | 789 | 177263 | 8451 | 75 | 0 | 4322 | 20000 | pass | 6.6M | 18 | 23 | 508 | 99.7% | 5054 | 6000 | 0 | complete | false | `tokio/src/lib.rs` |

## Details

## express

- URL: https://github.com/expressjs/express.git
- Commit: `ba006766fb964571723138708eacaba0f55759cd`
- Indexed files: 141
- Symbols: 2428
- Duration: 5925 ms
- Index budget: 10000 ms (pass)
- Context seed file: `lib/application.js`
- Context task: understand express application routing behavior
- Context files: 4
- Context ranges: 13
- Context lines: 362 of 21440 (98.3% reduction)
- Context estimated tokens: 2360
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
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
- Duration: 588 ms
- Index budget: 5000 ms (pass)
- Context seed file: `src/flask/app.py`
- Context task: understand flask application dispatch behavior
- Context files: 12
- Context ranges: 15
- Context lines: 573 of 18337 (96.9% reduction)
- Context estimated tokens: 5851
- Context applied token budget: 6000
- Context omitted files: 2
- Context omitted ranges: 2
- Context truncation reason: token_budget_exhausted
- Context continuation status: omitted_candidates_available
- Context truncated: true

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `src/flask/app.py` | 4 | 1-40 | high |
| `src/flask/globals.py` | 1 | 1-40 | high |
| `src/flask/__init__.py` | 1 | 1-39 | medium |
| `src/flask/cli.py` | 1 | 1-40 | medium |
| `src/flask/ctx.py` | 1 | 1-40 | medium |
| `src/flask/debughelpers.py` | 1 | 1-40 | medium |
| `src/flask/helpers.py` | 1 | 1-40 | medium |
| `src/flask/sansio/app.py` | 1 | 1-40 | medium |
| `src/flask/sessions.py` | 1 | 1-40 | medium |
| `src/flask/signals.py` | 1 | 1-17 | medium |
| `src/flask/templating.py` | 1 | 1-40 | medium |
| `src/flask/testing.py` | 1 | 1-40 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 83 | 18337 |

## gin

- URL: https://github.com/gin-gonic/gin.git
- Commit: `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd`
- Indexed files: 99
- Symbols: 1857
- Duration: 611 ms
- Index budget: 5000 ms (pass)
- Context seed file: `gin.go`
- Context task: understand gin engine routing behavior
- Context files: 4
- Context ranges: 12
- Context lines: 305 of 24099 (98.7% reduction)
- Context estimated tokens: 2969
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `gin.go` | 9 | 1-7 | high |
| `internal/bytesconv/bytesconv.go` | 1 | 1-21 | high |
| `internal/fs/fs.go` | 1 | 1-21 | medium |
| `render/bson.go` | 1 | 1-34 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| go | 99 | 24099 |

## tokio

- URL: https://github.com/tokio-rs/tokio.git
- Commit: `9c465e2f427f12999054bf086682080764dd3364`
- Indexed files: 789
- Symbols: 8451
- Duration: 4322 ms
- Index budget: 20000 ms (pass)
- Context seed file: `tokio/src/lib.rs`
- Context task: understand tokio runtime public API
- Context files: 18
- Context ranges: 23
- Context lines: 508 of 177263 (99.7% reduction)
- Context estimated tokens: 5054
- Context applied token budget: 6000
- Context omitted files: 0
- Context omitted ranges: 0
- Context truncation reason: none
- Context continuation status: complete
- Context truncated: false

Context pack files:

| File | Ranges | First range | Importances |
| --- | ---: | --- | --- |
| `tokio/src/lib.rs` | 3 | 1-1 | high |
| `tokio/src/sync/watch.rs` | 3 | 1-40 | high, medium |
| `tokio/src/sync/once_cell.rs` | 2 | 353-357 | high |
| `tokio/src/sync/barrier.rs` | 1 | 138-142 | high |
| `tokio/src/sync/mpsc/bounded.rs` | 1 | 1271-1275 | high |
| `tokio/src/sync/mutex.rs` | 1 | 654-658 | high |
| `tokio/src/blocking.rs` | 1 | 1-40 | medium |
| `tokio/src/future/mod.rs` | 1 | 1-28 | medium |
| `tokio/src/loom/mod.rs` | 1 | 1-14 | medium |
| `tokio/src/sync/Notify.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/batch_semaphore.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/mpsc/chan.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/mpsc/error.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/notify.rs` | 1 | 1-40 | medium |
| `tokio/src/task/coop/mod.rs` | 1 | 1-40 | medium |
| `tokio/src/util/mod.rs` | 1 | 1-40 | medium |
| `tokio/src/util/trace.rs` | 1 | 1-40 | medium |
| `tokio/src/sync/tests/mod.rs` | 1 | 1-18 | medium |

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 789 | 177263 |
