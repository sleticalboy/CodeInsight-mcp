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

require_section_literal() {
  local file="$1"
  local section="$2"
  local literal="$3"
  local description="$4"

  if ! awk -v section="## $section" -v literal="$literal" '
    $0 == section { in_section = 1; next }
    in_section && /^## / { exit }
    in_section && index($0, literal) > 0 { found = 1; exit }
    END { exit(found ? 0 : 1) }
  ' "$ROOT_DIR/$file"; then
    echo "$file section $section is missing ${description}" >&2
    echo "literal: $literal" >&2
    exit 1
  fi
}

require_guardrail_row() {
  local file="$1"
  local repo="$2"
  local check="$3"
  local expectation="$4"

  require_section_literal "$file" "$repo" \
    "| \`$check\` | $expectation |" \
    "$check guardrail expectation"
}

require_context_guardrail_report_sync() {
  local profile="$1"
  local report="$2"
  local config line repo specs check key value

  config="$(
    CODEINSIGHT_BENCH_PROFILE="$profile" \
    CODEINSIGHT_BENCH_PRINT_CONFIG=1 \
    "$ROOT_DIR/scripts/benchmark-smoke.sh"
  )"

  while IFS=$'\t' read -r repo specs; do
    if [ "$repo" = "name" ]; then
      continue
    fi

    IFS="|" read -r -a checks <<<"$specs"
    for check in "${checks[@]}"; do
      key="${check%%:*}"
      value="${check#*:}"

      case "$key" in
        selected_files)
          require_guardrail_row "$report" "$repo" "selected_files" ">= $value"
          ;;
        selected_ranges)
          require_guardrail_row "$report" "$repo" "selected_ranges" ">= $value"
          ;;
        reading_plan_steps)
          require_guardrail_row "$report" "$repo" "reading_plan_steps" ">= $value"
          ;;
        max_tokens)
          require_guardrail_row "$report" "$repo" "estimated_tokens" "<= $value and applied budget"
          ;;
        min_line_reduction)
          require_guardrail_row "$report" "$repo" "line_reduction" ">= $value%"
          ;;
        *)
          echo "unknown context guardrail key in $profile config: $key" >&2
          exit 1
          ;;
      esac
    done
  done <<<"$config"
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
    'route `context_pack` first for 4/4 repositories' \
    "context_pack benchmark claim"
  require_pattern README.md \
    'Generated reports include a `Key Results` section' \
    "benchmark key results claim"
  require_pattern .github/workflows/ci.yml \
    'actions/upload-artifact@v7' \
    "Node.js 24 artifact upload action"
  require_pattern .github/workflows/ci.yml \
    'steps\.benchmark-artifact\.outputs\.artifact-url' \
    "benchmark artifact URL summary input"
  require_pattern .github/workflows/ci.yml \
    'scripts/benchmark-step-summary\.sh /tmp/codeinsight-benchmark-subset\.md codeinsight-benchmark-subset' \
    "benchmark step summary command"

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
  require_pattern docs/benchmark-v0.1.md \
    'Context pack guardrails:' \
    "smoke context guardrail section"
  require_pattern docs/benchmark-v0.1.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "smoke reading plan guardrail"
  require_context_guardrail_report_sync smoke docs/benchmark-v0.1.md

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
  require_pattern docs/benchmark-large.md \
    'Context pack guardrails:' \
    "large context guardrail section"
  require_pattern docs/benchmark-large.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "large reading plan guardrail"
  require_context_guardrail_report_sync large docs/benchmark-large.md
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-v0.1.md" smoke
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-large.md" large

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
  require_pattern docs/demo-script.md \
    'Evidence Cutaway' \
    "benchmark evidence demo cutaway"
  require_pattern docs/demo-script.md \
    'Smoke benchmark: context_pack first for 4/4 repositories, 99\.2% aggregate line reduction\.' \
    "smoke benchmark evidence line"
  require_pattern docs/demo-script.md \
    'Large benchmark: context_pack first for 4/4 repositories, 99\.3% aggregate line reduction\.' \
    "large benchmark evidence line"
  require_pattern docs/demo-script.md \
    'The `Key Results` section in each report is the stable evidence slide' \
    "benchmark key results demo evidence"
  require_pattern docs/adoption-checklist.md \
    'The generated reports include `Key Results`' \
    "adoption key results pass criterion"
  require_pattern docs/release-readiness.md \
    'README benchmark snapshot, \[Demo script\]\(demo-script\.md\), and benchmark' \
    "release readiness benchmark consistency gate"
  require_pattern docs/release-readiness.md \
    '\[Demo script\]\(demo-script\.md\) evidence cutaway' \
    "release readiness demo evidence update target"
  require_pattern docs/release-readiness.md \
    'report `Key Results` should agree on `context_pack` first-tool' \
    "release readiness key results consistency"
  require_pattern docs/release-readiness.md \
    '`benchmark-subset-smoke` job summary' \
    "release readiness CI benchmark summary guidance"
  require_pattern docs/release-readiness.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "release readiness benchmark artifact download"
  require_pattern docs/maintenance-commands.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "maintenance benchmark artifact download"
  require_pattern docs/release-commands.md \
    'scripts/release-pretag-check\.sh main' \
    "release commands benchmark artifact gate"
  require_pattern docs/release-commands.md \
    'scripts/release-tag-preflight\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release commands tag preflight gate"
  require_pattern docs/release-commands.md \
    'remote tag already exists' \
    "release commands remote tag conflict gate"
  require_pattern docs/release-commands.md \
    'scripts/release-pretag-check\.sh --repo sleticalboy/CodeInsight-mcp --head-sha <tag-target-sha> main' \
    "release commands tag SHA benchmark artifact gate"
  require_pattern docs/release-commands.md \
    '^## Short Path$' \
    "release commands short path section"
  require_pattern docs/release-commands.md \
    'scripts/post-release-verify\.sh vX\.Y\.Z' \
    "release commands short path post-release verification"
  require_pattern docs/release-runbook.md \
    'scripts/release-pretag-check\.sh main' \
    "release runbook benchmark artifact gate"
  require_pattern docs/release-runbook.md \
    'scripts/release-tag-preflight\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release runbook tag preflight gate"
  require_pattern docs/release-runbook.md \
    'GitHub Release already exists for the tag' \
    "release runbook release conflict gate"
  require_pattern docs/release-runbook.md \
    'verify-pretag-ci' \
    "release runbook tag pretag workflow gate"
  require_pattern scripts/release-pretag-check.sh \
    'gh run watch "\$RUN_ID".*--exit-status' \
    "release pretag CI watch"
  require_pattern scripts/release-pretag-check.sh \
    '\-\-head-sha SHA' \
    "release pretag head SHA option"
  require_pattern scripts/release-pretag-check.sh \
    'BENCHMARK_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag artifact smoke hook"
  require_pattern docs/maintainer-checklist.md \
    'README benchmark snapshot, \[Demo script\]\(demo-script\.md\)' \
    "maintainer benchmark evidence consistency"
  require_pattern docs/maintainer-checklist.md \
    'benchmark report `Key Results`, and' \
    "maintainer key results consistency"
  require_pattern docs/maintainer-checklist.md \
    '\[Release readiness\]\(release-readiness\.md\) benchmark gate' \
    "maintainer release readiness consistency"
  require_pattern docs/maintainer-checklist.md \
    '`benchmark-subset-smoke` job summary' \
    "maintainer CI benchmark summary guidance"
  require_pattern docs/maintainer-checklist.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "maintainer benchmark artifact smoke command"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    'gh run download "\$RUN_ID"' \
    "benchmark artifact gh download command"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    '\-\-latest-success BRANCH' \
    "benchmark artifact latest successful run option"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    'benchmark-report-smoke\.sh' \
    "benchmark artifact report validation"
  require_pattern scripts/agent-router-demo.sh \
    'reading_plan_steps' \
    "agent-router reading plan output"
  require_pattern scripts/agent-router-demo.sh \
    'first_next_action' \
    "agent-router next action output"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_number_gt_zero "\$context_json" '\''\.reading_plan \| length'\''' \
    "agent-router reading plan assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.next_action'\''' \
    "agent-router next action assertion"

  echo "docs benchmark smoke passed"
}

main "$@"
