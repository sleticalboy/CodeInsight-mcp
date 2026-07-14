#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_pattern() {
  local file="$1"
  local pattern="$2"
  local description="$3"

  if ! grep -Eq "$pattern" "$ROOT_DIR/$file"; then
    echo "$file is missing ${description}" >&2
    echo "pattern: $pattern" >&2
    exit 1
  fi
}

main() {
  require_pattern README.md \
    '\[two-minute demo script\]\(docs/demo-script\.md\)' \
    "demo script link"
  require_pattern README.md \
    '\[Smoke benchmark\]\(docs/benchmark-v0\.1\.md\).*p-limit, itsdangerous, Go example,' \
    "smoke benchmark link and repository list"
  require_pattern README.md \
    '\[Large repository benchmark\]\(docs/benchmark-large\.md\).*express, Flask, Gin,' \
    "large benchmark link and repository list"
  require_pattern README.md \
    'CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke\.sh' \
    "large benchmark refresh command"
  require_pattern README.md \
    '`context_pack` as the first recommended action' \
    "context_pack benchmark claim"

  require_pattern scripts/benchmark-smoke.sh \
    'OUTPUT="\$\{CODEINSIGHT_BENCH_OUTPUT:-\$ROOT_DIR/docs/benchmark-v0\.1\.md\}"' \
    "smoke benchmark output path"
  require_pattern scripts/benchmark-smoke.sh \
    'OUTPUT="\$\{CODEINSIGHT_BENCH_OUTPUT:-\$ROOT_DIR/docs/benchmark-large\.md\}"' \
    "large benchmark output path"
  require_pattern scripts/benchmark-smoke.sh \
    '"p-limit"' \
    "p-limit fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"itsdangerous"' \
    "itsdangerous fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"go-example"' \
    "go-example fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"memchr"' \
    "memchr fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"express"' \
    "express fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"flask"' \
    "flask fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"gin"' \
    "gin fixture"
  require_pattern scripts/benchmark-smoke.sh \
    '"tokio"' \
    "tokio fixture"

  require_pattern docs/benchmark-v0.1.md \
    '^# CodeInsight v0\.1 Smoke Benchmark$' \
    "smoke benchmark title"
  require_pattern docs/benchmark-v0.1.md \
    'Profile: `smoke`' \
    "smoke benchmark profile"
  require_pattern docs/benchmark-v0.1.md \
    '6000 token budget' \
    "smoke benchmark token budget"
  require_pattern docs/benchmark-v0.1.md \
    '\| p-limit \| TypeScript \|' \
    "p-limit summary row"
  require_pattern docs/benchmark-v0.1.md \
    '\| itsdangerous \| Python \|' \
    "itsdangerous summary row"
  require_pattern docs/benchmark-v0.1.md \
    '\| go-example \| Go \|' \
    "go-example summary row"
  require_pattern docs/benchmark-v0.1.md \
    '\| memchr \| Rust \|' \
    "memchr summary row"
  require_pattern docs/benchmark-v0.1.md \
    '\| `context_pack` \|' \
    "context_pack recommended tool evidence"

  require_pattern docs/benchmark-large.md \
    '^# CodeInsight v0\.1 Large Repository Benchmark$' \
    "large benchmark title"
  require_pattern docs/benchmark-large.md \
    'Profile: `large`' \
    "large benchmark profile"
  require_pattern docs/benchmark-large.md \
    '6000 token budget' \
    "large benchmark token budget"
  require_pattern docs/benchmark-large.md \
    '\| express \| JavaScript \|' \
    "express summary row"
  require_pattern docs/benchmark-large.md \
    '\| flask \| Python \|' \
    "flask summary row"
  require_pattern docs/benchmark-large.md \
    '\| gin \| Go \|' \
    "gin summary row"
  require_pattern docs/benchmark-large.md \
    '\| tokio \| Rust \|' \
    "tokio summary row"
  require_pattern docs/benchmark-large.md \
    '\| `context_pack` \|' \
    "context_pack recommended tool evidence"

  require_pattern docs/demo-script.md \
    'scripts/agent-router-demo\.sh' \
    "agent-router demo command"
  require_pattern docs/demo-script.md \
    'index_project' \
    "index_project demo stage"
  require_pattern docs/demo-script.md \
    'project_overview' \
    "project_overview demo stage"
  require_pattern docs/demo-script.md \
    'context_pack' \
    "context_pack demo stage"
  require_pattern docs/demo-script.md \
    'reading_plan_steps' \
    "reading plan demo metric"
  require_pattern docs/demo-script.md \
    'first_next_action' \
    "reading plan next action demo metric"
  require_pattern docs/demo-script.md \
    'impact_analysis' \
    "impact_analysis demo stage"
  require_pattern scripts/agent-router-demo.sh \
    'reading_plan_steps' \
    "agent-router reading plan output"
  require_pattern scripts/agent-router-demo.sh \
    'first_next_action' \
    "agent-router next action output"

  echo "docs benchmark smoke passed"
}

main "$@"
