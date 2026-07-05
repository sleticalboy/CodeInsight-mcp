# CodeInsight v0.1 Smoke Benchmark

Generated at: 2026-07-05 10:19:31 UTC

This is a smoke benchmark, not a controlled performance benchmark. It verifies
that CodeInsight can index real public repositories across the MVP language set
and produce stable project summaries without crashing.

Environment:

- Command: `target/release/codeinsight`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | DB size |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| p-limit | TypeScript | `42599eb` | 6 | 1123 | 162 | 10 | 0 | 36 | 132K |
| itsdangerous | Python | `672971d` | 15 | 1712 | 144 | 35 | 0 | 38 | 132K |
| go-example | Go | `7f05d21` | 38 | 3537 | 189 | 33 | 0 | 74 | 168K |
| memchr | Rust | `e21e9fb` | 64 | 69365 | 4045 | 100 | 0 | 726 | 1.7M |

## Details

## p-limit

- URL: https://github.com/sindresorhus/p-limit.git
- Commit: `42599ebbbb1228a5bdab381fcf8f4ac20eb8d551`
- Indexed files: 6
- Symbols: 162
- Duration: 36 ms

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
- Duration: 38 ms

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| python | 15 | 1712 |

## go-example

- URL: https://github.com/golang/example.git
- Commit: `7f05d217867b2af52b0a28c6d1c91df97e1b5b39`
- Indexed files: 38
- Symbols: 189
- Duration: 74 ms

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
- Duration: 726 ms

Language breakdown:

| Language | Files | Lines |
| --- | ---: | ---: |
| rust | 64 | 69365 |
