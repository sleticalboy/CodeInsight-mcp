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
    '\[Adoption cases\]\(docs/adoption-cases\.md\)' \
    "README adoption cases summary link"
  require_pattern README.md \
    '\[CodeInsight self adoption report\]\(docs/adoption-report-codeinsight\.md\)' \
    "README self adoption report link"
  require_pattern README.md \
    '1,585 of 126,990 source lines' \
    "README adoption cases aggregate snapshot"
  require_pattern README.md \
    '80\.1x aggregate read-less ratio' \
    "README adoption cases aggregate read-less ratio"
  require_pattern README.md \
    'routes the entrypoint task to 439 of 28,433 source lines, a 98\.5% first-read' \
    "README self adoption report metric"
  "$ROOT_DIR/scripts/readme-adoption-summary-smoke.sh" >/dev/null
  require_pattern README.md \
    '\[Benchmark methodology\]\(docs/benchmark-methodology\.md\)' \
    "benchmark methodology README link"
  require_pattern README.md \
    'These are first-read routing reports, not controlled performance claims' \
    "README first-read routing evidence caveat"
  require_pattern README.md \
    'agent workflow focus and token discipline' \
    "README adoption evidence interpretation"
  require_pattern README.md \
    'not as runtime performance, parser' \
    "README adoption non-performance caveat"
  require_pattern README.md \
    'CODEINSIGHT_BENCH_PROFILE=large scripts/benchmark-smoke\.sh' \
    "large benchmark refresh command"
  require_pattern README.md \
    'CODEINSIGHT_BENCH_LOCAL_ROOT=/path/to/repo' \
    "README local benchmark command"
  require_pattern README.md \
    'Adoption evidence snippet for your own repository:' \
    "README adoption evidence snippet heading"
  require_pattern README.md \
    'scripts/adoption-evidence\.sh /path/to/repo' \
    "README adoption evidence command"
  require_pattern README.md \
    'scripts/adoption-comparison\.sh /path/to/repo' \
    "README adoption comparison command"
  require_pattern README.md \
    'blind-read vs routed-first-read comparison' \
    "README adoption comparison description"
  require_pattern README.md \
    '\-\-print-snippet' \
    "README adoption evidence print snippet option"
  require_pattern README.md \
    '\-\-issue-template' \
    "README adoption evidence issue template option"
  require_pattern README.md \
    'issue-template\.md' \
    "README adoption issue template artifact"
  require_pattern README.md \
    'scripts/adoption-report\.sh /path/to/repo' \
    "README adoption report command"
  require_pattern README.md \
    'codeinsight-adoption-report\.tar\.gz' \
    "README adoption report archive artifact"
  require_pattern README.md \
    'prints this copyable shape' \
    "README adoption printed snippet guidance"
  require_pattern README.md \
    'MCP suggested tool executed: `true`' \
    "README adoption MCP suggested tool signal"
  require_pattern README.md \
    'MCP first-call contract: reading_order=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`' \
    "README adoption MCP first-call contract signal"
  require_pattern README.md \
    'Use `summary\.json` from the same folder' \
    "README adoption summary JSON guidance"
  require_pattern docs/adoption-checklist.md \
    '\-\-issue-template' \
    "adoption checklist issue template option"
  require_pattern docs/adoption-checklist.md \
    'failure category placeholder' \
    "adoption checklist issue template contents"
  require_pattern docs/adoption-checklist.md \
    'scripts/adoption-report\.sh /path/to/repo' \
    "adoption checklist report command"
  require_pattern docs/adoption-checklist.md \
    'scripts/adoption-comparison\.sh /path/to/repo' \
    "adoption checklist comparison command"
  require_pattern docs/adoption-checklist.md \
    'source lines avoided, read-less ratio' \
    "adoption checklist comparison metrics"
  require_pattern docs/adoption-checklist.md \
    '\[Adoption cases\]\(adoption-cases\.md\)' \
    "adoption checklist adoption cases summary link"
  require_pattern docs/adoption-checklist.md \
    '\[Express adoption case\]\(adoption-case-express\.md\)' \
    "adoption checklist Express case link"
  require_pattern docs/adoption-checklist.md \
    '\[Gin adoption case\]\(adoption-case-gin\.md\)' \
    "adoption checklist Gin case link"
  require_pattern docs/adoption-checklist.md \
    '\[Memchr adoption case\]\(adoption-case-memchr\.md\)' \
    "adoption checklist Memchr case link"
  require_pattern docs/adoption-checklist.md \
    '\[Requests adoption case\]\(adoption-case-requests\.md\)' \
    "adoption checklist Requests case link"
  require_pattern docs/adoption-checklist.md \
    '\[CodeInsight self adoption report\]\(adoption-report-codeinsight\.md\)' \
    "adoption checklist self report link"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh express' \
    "adoption checklist Express refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh gin' \
    "adoption checklist Gin refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh memchr' \
    "adoption checklist Memchr refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh requests' \
    "adoption checklist Requests refresh command"
  require_pattern docs/adoption-checklist.md \
    'codeinsight-adoption-report\.tar\.gz' \
    "adoption checklist report archive artifact"
  require_pattern docs/quickstart.md \
    'You need adoption comparison evidence' \
    "quickstart adoption comparison row"
  require_pattern docs/quickstart.md \
    'You want evidence for your own repository' \
    "quickstart local benchmark row"
  require_pattern docs/README.md \
    'Adoption comparison evidence' \
    "docs index adoption comparison validation row"
  require_pattern docs/README.md \
    '\[Adoption cases\]\(adoption-cases\.md\)' \
    "docs index adoption cases summary link"
  require_pattern docs/README.md \
    '\[CodeInsight self adoption report\]\(adoption-report-codeinsight\.md\)' \
    "docs index self adoption report link"
  require_pattern docs/README.md \
    'Uploadable adoption report' \
    "docs index uploadable adoption report validation row"
  require_pattern docs/README.md \
    '\[Express adoption case\]\(adoption-case-express\.md\)' \
    "docs index Express adoption case link"
  require_pattern docs/README.md \
    '\[Gin adoption case\]\(adoption-case-gin\.md\)' \
    "docs index Gin adoption case link"
  require_pattern docs/README.md \
    '\[Memchr adoption case\]\(adoption-case-memchr\.md\)' \
    "docs index Memchr adoption case link"
  require_pattern docs/README.md \
    '\[Requests adoption case\]\(adoption-case-requests\.md\)' \
    "docs index Requests adoption case link"
  require_pattern docs/README.md \
    'Local repository benchmark' \
    "docs index local benchmark validation row"
  require_pattern docs/adoption-case-express.md \
    'Commit: `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4`' \
    "Express adoption case commit"
  require_pattern docs/adoption-case-express.md \
    'Read less | `92\.6x`' \
    "Express adoption case read-less metric"
  require_pattern docs/adoption-case-express.md \
    'Generated with: `scripts/update-adoption-case\.sh express`' \
    "Express adoption case generator"
  require_pattern docs/adoption-case-express.md \
    'Refresh this checked-in snapshot:' \
    "Express adoption case refresh section"
  require_pattern docs/adoption-case-express.md \
    'scripts/update-adoption-case\.sh express --commit ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4' \
    "Express adoption case exact refresh command"
  require_pattern docs/adoption-case-express.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-express' \
    "Express adoption case reproduce command"
  require_pattern docs/adoption-case-gin.md \
    'Commit: `34dac209ffb6ef85cc78c5d217bbb7ad001d68fd`' \
    "Gin adoption case commit"
  require_pattern docs/adoption-case-gin.md \
    'Read less | `51\.1x`' \
    "Gin adoption case read-less metric"
  require_pattern docs/adoption-case-gin.md \
    'Generated with: `scripts/update-adoption-case\.sh gin`' \
    "Gin adoption case generator"
  require_pattern docs/adoption-case-gin.md \
    'scripts/update-adoption-case\.sh gin --commit 34dac209ffb6ef85cc78c5d217bbb7ad001d68fd' \
    "Gin adoption case exact refresh command"
  require_pattern docs/adoption-case-gin.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-gin' \
    "Gin adoption case reproduce command"
  require_pattern docs/adoption-case-memchr.md \
    'Commit: `bce7df7140acff420478a358cde5587904000cb1`' \
    "Memchr adoption case commit"
  require_pattern docs/adoption-case-memchr.md \
    'Read less | `301\.7x`' \
    "Memchr adoption case read-less metric"
  require_pattern docs/adoption-case-memchr.md \
    'Generated with: `scripts/update-adoption-case\.sh memchr`' \
    "Memchr adoption case generator"
  require_pattern docs/adoption-case-memchr.md \
    'scripts/update-adoption-case\.sh memchr --commit bce7df7140acff420478a358cde5587904000cb1' \
    "Memchr adoption case exact refresh command"
  require_pattern docs/adoption-case-memchr.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-memchr' \
    "Memchr adoption case reproduce command"
  require_pattern docs/adoption-case-requests.md \
    'Commit: `f361ead047be5cb873174218582f7d8b9fcd9f49`' \
    "Requests adoption case commit"
  require_pattern docs/adoption-case-requests.md \
    'Read less | `18\.5x`' \
    "Requests adoption case read-less metric"
  require_pattern docs/adoption-case-requests.md \
    'Generated with: `scripts/update-adoption-case\.sh requests`' \
    "Requests adoption case generator"
  require_pattern docs/adoption-case-requests.md \
    'scripts/update-adoption-case\.sh requests --commit f361ead047be5cb873174218582f7d8b9fcd9f49' \
    "Requests adoption case exact refresh command"
  require_pattern docs/adoption-case-requests.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-requests' \
    "Requests adoption case reproduce command"
  require_pattern docs/adoption-cases.md \
    'Blind first-read baseline: `126,990` source lines' \
    "adoption cases aggregate baseline"
  require_pattern docs/adoption-cases.md \
    'Aggregate first-read reduction: `98\.8%`' \
    "adoption cases aggregate reduction"
  require_pattern docs/adoption-cases.md \
    'Aggregate read-less ratio: `80\.1x`' \
    "adoption cases aggregate read-less ratio"
  require_pattern docs/adoption-cases.md \
    '^## How To Read These Numbers$' \
    "adoption cases interpretation section"
  require_pattern docs/adoption-cases.md \
    'The baseline is the number of indexed source lines an agent could read' \
    "adoption cases baseline definition"
  require_pattern docs/adoption-cases.md \
    'describe first-read context routing, not runtime performance' \
    "adoption cases routing caveat"
  require_pattern docs/adoption-cases.md \
    'code conclusions still need normal local verification' \
    "adoption cases verification caveat"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-express\.md\)' \
    "adoption cases Express detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-gin\.md\)' \
    "adoption cases Gin detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-memchr\.md\)' \
    "adoption cases Memchr detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-requests\.md\)' \
    "adoption cases Requests detail link"
  require_pattern docs/adoption-report-codeinsight.md \
    'CodeInsight routed first-read \| `439` source lines' \
    "CodeInsight self adoption report routed lines"
  require_pattern docs/adoption-report-codeinsight.md \
    'First-read reduction \| `98\.5%`' \
    "CodeInsight self adoption report reduction"
  require_pattern docs/adoption-report-codeinsight.md \
    'Reading order starts with selected context \| `true`' \
    "CodeInsight self adoption report reading order contract"
  require_pattern docs/adoption-report-codeinsight.md \
    'Suggested tool executed through MCP `tools/call` \| `true`' \
    "CodeInsight self adoption report suggested tool contract"
  require_pattern docs/adoption-report-codeinsight.md \
    '/tmp/codeinsight-self-adoption-report\.tar\.gz' \
    "CodeInsight self adoption report archive path"
  require_pattern docs/adoption-report-codeinsight.md \
    'mcp-first-call\.json' \
    "CodeInsight self adoption report MCP artifact"
  "$ROOT_DIR/scripts/update-adoption-cases.sh" --check >/dev/null
  require_pattern scripts/update-adoption-case.sh \
    'Refreshes a checked-in adoption case from a live adoption-comparison run' \
    "Express adoption case update script purpose"
  require_pattern scripts/update-adoption-case.sh \
    'gin\)' \
    "Gin adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'memchr\)' \
    "Memchr adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'requests\)' \
    "Requests adoption case update script branch"
  require_pattern README.md \
    'route `context_pack` first for 4/4 repositories' \
    "context_pack benchmark claim"
  require_pattern README.md \
    'Generated reports include a `Key Results` section' \
    "benchmark key results claim"
  require_pattern docs/README.md \
    '\[Benchmark methodology\]\(benchmark-methodology\.md\)' \
    "docs index benchmark methodology link"
  require_pattern docs/benchmark-methodology.md \
    'CodeInsight benchmark reports are reproducible evidence for the AI-agent' \
    "benchmark methodology purpose"
  require_pattern docs/benchmark-methodology.md \
    'CODEINSIGHT_BENCH_REPOS=p-limit scripts/benchmark-smoke\.sh' \
    "benchmark methodology subset command"
  require_pattern docs/benchmark-methodology.md \
    'CODEINSIGHT_BENCH_PROFILE=local' \
    "benchmark methodology local profile command"
  require_pattern docs/benchmark-methodology.md \
    'prints a terminal summary with the report path' \
    "benchmark methodology terminal summary"
  require_pattern docs/benchmark-methodology.md \
    'CODEINSIGHT_BENCH_SUMMARY_JSON=/tmp/codeinsight-local-summary\.json' \
    "benchmark methodology summary JSON command"
  require_pattern docs/benchmark-methodology.md \
    'scripts/benchmark-summary-text\.sh /tmp/codeinsight-local-summary\.json' \
    "benchmark methodology summary text command"
  require_pattern scripts/benchmark-smoke.sh \
    'CODEINSIGHT_BENCH_LOCAL_ROOT is required when CODEINSIGHT_BENCH_PROFILE=local' \
    "benchmark local profile root validation"
  require_pattern scripts/benchmark-smoke.sh \
    'wrote summary \$SUMMARY_JSON' \
    "benchmark summary JSON output"
  require_pattern scripts/benchmark-smoke.sh \
    'continue with: file_outline for first files, dependency_graph for imports, impact_analysis before edits' \
    "benchmark terminal next steps"
  require_pattern scripts/benchmark-local-smoke.sh \
    'CODEINSIGHT_BENCH_PROFILE=local' \
    "benchmark local smoke profile"
  require_pattern scripts/benchmark-summary-text.sh \
    'CodeInsight Benchmark Summary' \
    "benchmark summary text heading"
  require_pattern scripts/benchmark-summary-text-smoke.sh \
    'Routing: `context_pack` first for 1/1 repositories' \
    "benchmark summary text smoke routing"
  require_pattern docs/benchmark-methodology.md \
    'Every report also checks that:' \
    "benchmark methodology guardrails"
  require_pattern .github/workflows/ci.yml \
    'actions/upload-artifact@v7' \
    "Node.js 24 artifact upload action"
  require_pattern .github/workflows/ci.yml \
    'steps\.benchmark-artifact\.outputs\.artifact-url' \
    "benchmark artifact URL summary input"
  require_pattern .github/workflows/ci.yml \
    'CODEINSIGHT_BENCH_SUMMARY_JSON: /tmp/codeinsight-benchmark-subset\.json' \
    "benchmark summary JSON CI output"
  require_pattern .github/workflows/ci.yml \
    'scripts/benchmark-step-summary\.sh /tmp/codeinsight-benchmark-subset\.md codeinsight-benchmark-subset' \
    "benchmark step summary command"
  require_pattern .github/workflows/ci.yml \
    '/tmp/codeinsight-benchmark-subset\.json' \
    "benchmark summary JSON CI input"
  require_pattern scripts/benchmark-step-summary.sh \
    'benchmark-summary-text\.sh' \
    "benchmark step summary compact JSON renderer"
  require_pattern .github/workflows/ci.yml \
    'context-pack-quality-smoke:' \
    "context-pack quality CI job"
  require_pattern .github/workflows/ci.yml \
    'codeinsight-context-pack-quality' \
    "context-pack quality artifact"
  require_pattern .github/workflows/ci.yml \
    'scripts/context-pack-quality-step-summary\.sh /tmp/codeinsight-context-pack-quality\.json codeinsight-context-pack-quality' \
    "context-pack quality step summary command"

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
  require_pattern scripts/benchmark-smoke.sh \
    'first_reading_question' \
    "benchmark first reading question guardrail"
  require_pattern scripts/benchmark-smoke.sh \
    '\| File \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "benchmark reading-plan question column"

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
    'Context reading plan:' \
    "smoke context reading plan section"
  require_pattern docs/benchmark-v0.1.md \
    '\| File \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "smoke context reading plan question column"
  require_pattern docs/benchmark-v0.1.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "smoke reading plan guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_reading_question` \| present \|' \
    "smoke reading question guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_reading_reason` \| present \|' \
    "smoke reading reason guardrail"
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
    'Context reading plan:' \
    "large context reading plan section"
  require_pattern docs/benchmark-large.md \
    '\| File \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "large context reading plan question column"
  require_pattern docs/benchmark-large.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "large reading plan guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_reading_question` \| present \|' \
    "large reading question guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_reading_reason` \| present \|' \
    "large reading reason guardrail"
  require_context_guardrail_report_sync large docs/benchmark-large.md
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-v0.1.md" smoke
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-large.md" large

  require_pattern docs/demo-script.md \
    'scripts/two-minute-demo\.sh' \
    "two-minute demo command"
  require_pattern docs/demo-script.md \
    'scripts/agent-router-demo\.sh' \
    "agent-router raw metrics command"
  require_pattern README.md \
    'scripts/two-minute-demo\.sh' \
    "README two-minute demo command"
  require_pattern README.md \
    'Pick the validation that matches your adoption stage:' \
    "README validation chooser"
  require_pattern README.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/mcp-first-call-smoke\.sh' \
    "README MCP first-call smoke command"
  require_pattern README.md \
    'returns the first context file, follows `reading_plan\[\]`, runs the current step'\''s suggested tool' \
    "README MCP first-call value proof"
  require_pattern README.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "README installed adoption coverage"
  require_pattern docs/quickstart.md \
    'scripts/two-minute-demo\.sh' \
    "quickstart two-minute demo command"
  require_pattern docs/adoption-checklist.md \
    'scripts/two-minute-demo\.sh' \
    "adoption two-minute demo command"
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
  require_pattern docs/release-readiness.md \
    'scripts/mcp-first-call-artifact-smoke\.sh <ci-run-id>' \
    "release readiness MCP first-call artifact download"
  require_pattern docs/maintenance-commands.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "maintenance benchmark artifact download"
  require_pattern docs/maintenance-commands.md \
    'scripts/context-pack-quality-artifact-smoke\.sh <ci-run-id>' \
    "maintenance context-pack quality artifact download"
  require_pattern docs/maintenance-commands.md \
    'scripts/mcp-first-call-artifact-smoke\.sh <ci-run-id>' \
    "maintenance MCP first-call artifact download"
  require_pattern docs/maintenance-commands.md \
    'context, first reading question, token-budget, and impact metrics' \
    "maintenance agent-route first reading question summary"
  require_pattern docs/maintenance-commands.md \
    'scripts/release-evidence-summary-artifact-smoke\.sh --repo sleticalboy/CodeInsight-mcp <ci-run-id>' \
    "maintenance release evidence summary artifact smoke command"
  require_pattern docs/maintenance-commands.md \
    'scripts/archive-release-evidence\.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX\.Y\.Z\.json vX\.Y\.Z main' \
    "maintenance archive release evidence command"
  require_pattern docs/maintenance-commands.md \
    '^## Recommended Release Path$' \
    "maintenance recommended release path section"
  require_pattern README.md \
    'Maintainer release path:' \
    "README maintainer release path"
  require_pattern README.md \
    'scripts/archive-release-evidence\.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX\.Y\.Z\.json vX\.Y\.Z main' \
    "README archive release evidence command"
  require_pattern docs/maintainer-checklist.md \
    'scripts/archive-release-evidence\.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX\.Y\.Z\.json vX\.Y\.Z main' \
    "maintainer checklist archive release evidence command"
  require_pattern docs/maintenance-commands.md \
    'scripts/context-pack-quality-smoke\.sh' \
    "maintenance context-pack quality smoke"
  require_pattern docs/maintenance-commands.md \
    'Choose the narrowest check for the change:' \
    "maintenance smoke check chooser"
  require_pattern docs/maintenance-commands.md \
    '\| README/demo positioning changed \| `scripts/two-minute-demo\.sh` and `scripts/demo-output-smoke\.sh` \|' \
    "maintenance README demo smoke chooser"
  require_pattern docs/maintenance-commands.md \
    '\| First MCP call onboarding changed \| `scripts/mcp-first-call-smoke\.sh --summary-json /tmp/codeinsight-mcp-first-call\.json` \|' \
    "maintenance MCP first-call smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'first context file, reading-plan order, suggested-tool handoff, impact status, and saved artifacts' \
    "maintenance MCP first-call artifact scope"
  require_pattern docs/maintenance-commands.md \
    'scripts/mcp-first-call-step-summary-smoke\.sh' \
    "maintenance MCP first-call step summary smoke"
  require_pattern docs/maintenance-commands.md \
    '\| First MCP call Actions summary changed \| `scripts/mcp-first-call-step-summary-smoke\.sh` \|' \
    "maintenance MCP first-call step summary chooser"
  require_pattern docs/maintenance-commands.md \
    'Actions Summary section for selected files, first context file, first reading file, reading-plan order, suggested-tool handoff, continuation timing, impact status, and artifact link' \
    "maintenance MCP first-call step summary scope"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First context file: `src/main\.ts`' \
    "MCP first-call step summary first context file"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Reading order contract: `true`' \
    "MCP first-call step summary reading order contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Suggested tool handoff contract: `true`' \
    "MCP first-call step summary suggested tool handoff contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Continuation timing contract: `true`' \
    "MCP first-call step summary continuation timing contract"
  require_pattern docs/maintenance-commands.md \
    '\| First MCP call help or failure messaging changed \| `scripts/mcp-first-call-failure-smoke\.sh` \|' \
    "maintenance MCP first-call failure smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'Fast checks for `--help`, `\[usage\]`, `\[binary\]`, and `\[mcp_server\]` output\.' \
    "maintenance MCP first-call failure smoke scope"
  require_pattern docs/maintenance-commands.md \
    '\| MCP protocol or tool payload changed \| `scripts/mcp-stdio-smoke\.sh` \|' \
    "maintenance MCP smoke chooser"
  require_pattern docs/maintenance-commands.md \
    '\| Installed-binary adoption path changed \| `CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh` \|' \
    "maintenance installed binary smoke chooser"
  require_pattern docs/maintenance-commands.md \
    '\| One-call `agent_route` JSON contract changed \| `scripts/agent-route-smoke\.sh` \|' \
    "maintenance agent route smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'production-vs-test' \
    "maintenance context-pack quality scope"
  require_pattern docs/maintenance-commands.md \
    'dependency continuation' \
    "maintenance context-pack dependency continuation scope"
  require_pattern docs/maintenance-commands.md \
    'omitted_candidates' \
    "maintenance context-pack omitted candidates scope"
  require_pattern docs/maintenance-commands.md \
    'minimum_budget_applied' \
    "maintenance context-pack minimum budget scope"
  require_pattern docs/maintenance-commands.md \
    'token_budget_exhausted' \
    "maintenance context-pack token exhaustion scope"
  require_pattern docs/maintenance-commands.md \
    '\-\-summary-json <path>' \
    "maintenance context-pack summary JSON scope"
  require_pattern docs/maintenance-commands.md \
    'codeinsight-context-pack-quality' \
    "maintenance context-pack quality artifact scope"
  require_pattern docs/maintenance-commands.md \
    'first reading question metrics for selected context' \
    "maintenance context-pack quality first reading question scope"
  require_pattern docs/maintainer-checklist.md \
    'context-pack quality smoke' \
    "maintainer context-pack quality smoke"
  require_pattern docs/maintainer-checklist.md \
    'first reading question metrics' \
    "maintainer context-pack quality first reading question summary"
  require_pattern docs/maintainer-checklist.md \
    'context-pack-quality-smoke` job summary' \
    "maintainer context-pack quality CI summary guidance"
  require_pattern docs/maintainer-checklist.md \
    'scripts/context-pack-quality-artifact-smoke\.sh <ci-run-id>' \
    "maintainer context-pack quality artifact smoke command"
  require_pattern docs/maintainer-checklist.md \
    'scripts/mcp-first-call-artifact-smoke\.sh <ci-run-id>' \
    "maintainer MCP first-call artifact smoke command"
  require_pattern docs/maintainer-checklist.md \
    'context-pack metrics, first reading question, impact metrics' \
    "maintainer agent-route first reading question summary"
  require_pattern scripts/context-pack-quality-step-summary-smoke.sh \
    'codeinsight-context-pack-quality' \
    "context-pack quality step summary smoke artifact"
  require_pattern scripts/context-pack-quality-artifact-smoke.sh \
    'gh run download "\$RUN_ID"' \
    "context-pack quality artifact gh download command"
  require_pattern scripts/context-pack-quality-artifact-smoke.sh \
    '\-\-latest-success BRANCH' \
    "context-pack quality artifact latest successful run option"
  require_pattern scripts/context-pack-quality-artifact-smoke.sh \
    'omitted_candidates_available' \
    "context-pack quality artifact metric validation"
  require_pattern scripts/context-pack-quality-artifact-smoke.sh \
    'first_reading_question' \
    "context-pack quality artifact first reading question output"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'gh run download "\$RUN_ID"' \
    "MCP first-call artifact gh download command"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    '\-\-latest-success BRANCH' \
    "MCP first-call artifact latest successful run option"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'suggested_tool_executed == true' \
    "MCP first-call artifact suggested tool validation"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'execution_plan_reads_in_reading_plan_order == true' \
    "MCP first-call artifact reading order validation"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'current_step_suggested_tool_matches_reading_plan == true' \
    "MCP first-call artifact suggested tool handoff validation"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'first_reading_question' \
    "MCP first-call artifact first reading question output"
  require_pattern docs/maintainer-checklist.md \
    'the first context file, first reading file, first next action, reading-order' \
    "maintainer MCP first-call route contract summary"
  require_pattern docs/release-readiness.md \
    'first reading file, first next action, reading-order and suggested-tool handoff' \
    "release readiness MCP first-call route contract summary"
  require_pattern docs/release-commands.md \
    'scripts/release-pretag-check\.sh main' \
    "release commands benchmark artifact gate"
  require_pattern docs/release-commands.md \
    'scripts/release-dry-run\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release commands dry run orchestration"
  require_pattern docs/release-commands.md \
    'evidence-file release-evidence/vX\.Y\.Z\.md' \
    "release commands dry run evidence archive"
  require_pattern docs/release-commands.md \
    'evidence-json-file release-evidence/vX\.Y\.Z\.json' \
    "release commands dry run evidence JSON archive"
  require_pattern docs/release-commands.md \
    'scripts/release-tag-preflight\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release commands tag preflight gate"
  require_pattern docs/release-commands.md \
    'scripts/archive-release-evidence\.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX\.Y\.Z\.json vX\.Y\.Z main' \
    "release commands evidence archive"
  require_pattern docs/release-commands.md \
    'release-evidence/vX\.Y\.Z\.md' \
    "release commands evidence archive path"
  require_pattern docs/release-commands.md \
    'codeinsight-mcp-first-call' \
    "release commands MCP first-call evidence artifact"
  require_pattern docs/release-commands.md \
    '\[CodeInsight self adoption report\]\(adoption-report-codeinsight\.md\)' \
    "release commands adoption report handoff"
  require_pattern docs/release-commands.md \
    '/tmp/codeinsight-self-adoption-report\.tar\.gz' \
    "release commands adoption report archive"
  require_pattern docs/release-commands.md \
    '439/28433' \
    "release commands adoption report metric"
  require_pattern docs/release-commands.md \
    'machine-readable JSON' \
    "release commands machine-readable evidence JSON"
  require_pattern docs/release-commands.md \
    'scripts/update-release-status\.sh --evidence-json-file release-evidence/vX\.Y\.Z\.json release-verification/vX\.Y\.Z\.json' \
    "release commands status evidence JSON file"
  require_pattern docs/release-commands.md \
    'scripts/update-release-status\.sh --evidence-file release-evidence/vX\.Y\.Z\.md release-verification/vX\.Y\.Z\.json' \
    "release commands status evidence file"
  require_pattern docs/release-commands.md \
    'archived pre-release evidence fields' \
    "release commands archived evidence fields"
  require_pattern docs/release-commands.md \
    'including the adoption report document, reproduce command, archive path' \
    "release commands status adoption report fields"
  require_pattern docs/release-commands.md \
    'MCP first-call contract booleans when present' \
    "release commands status adoption report contract"
  require_pattern docs/release-commands.md \
    'falls back to `release-evidence/<tag>\.md`' \
    "release commands evidence markdown fallback"
  require_pattern docs/release-commands.md \
    'scripts/release-handoff-summary\.sh --json-output release-handoff/vX\.Y\.Z\.json --output release-handoff/vX\.Y\.Z\.md vX\.Y\.Z' \
    "release commands handoff summary"
  require_pattern docs/release-commands.md \
    'scripts/release-notes-draft\.sh --changelog-notes /tmp/codeinsight-release-notes\.md --output release-handoff/vX\.Y\.Z\.release-notes\.md vX\.Y\.Z' \
    "release commands handoff release notes draft"
  require_pattern docs/release-commands.md \
    'post-release verification' \
    "release commands handoff verification input"
  require_pattern docs/release-commands.md \
    'remote tag already exists' \
    "release commands remote tag conflict gate"
  require_pattern docs/release-commands.md \
    '`Cargo\.toml`, `docs/install\.md`, and `CHANGELOG\.md` are prepared for the same' \
    "release commands metadata consistency gate"
  require_pattern docs/release-commands.md \
    '`metadata_cargo`, `metadata_install`, and' \
    "release commands metadata summary output"
  require_pattern docs/release-commands.md \
    'release dry run checklist' \
    "release commands dry run checklist"
  require_pattern docs/release-commands.md \
    'artifact gates without modifying the checkout' \
    "release commands dry run checkout safety"
  require_pattern docs/release-runbook.md \
    'release dry run checklist' \
    "release runbook dry run checklist"
  require_pattern docs/release-runbook.md \
    'optional evidence file path' \
    "release runbook dry run checklist evidence file"
  require_pattern docs/release-commands.md \
    'scripts/release-pretag-check\.sh --repo sleticalboy/CodeInsight-mcp --head-sha <tag-target-sha> main' \
    "release commands tag SHA benchmark artifact gate"
  require_pattern docs/release-commands.md \
    '^## Short Path$' \
    "release commands short path section"
  require_pattern docs/release-commands.md \
    'three phases: dry-run evidence, prepare' \
    "release commands three-phase SOP"
  require_pattern docs/release-commands.md \
    '# 1\. Dry-run and archive pre-tag evidence\.' \
    "release commands dry-run phase"
  require_pattern docs/release-commands.md \
    '# 2\. Prepare and push the release metadata commit\.' \
    "release commands prepare phase"
  require_pattern docs/release-commands.md \
    '# 3\. Wait for CI, tag, then verify published artifacts\.' \
    "release commands tag phase"
  require_pattern docs/release-commands.md \
    'scripts/post-release-verify\.sh --handoff vX\.Y\.Z' \
    "release commands short path post-release verification handoff"
  require_pattern docs/release-runbook.md \
    '^## Recommended SOP$' \
    "release runbook recommended SOP section"
  require_pattern docs/release-runbook.md \
    'Normal releases should follow this three-phase flow' \
    "release runbook three-phase SOP"
  require_pattern docs/release-runbook.md \
    'scripts/release-pretag-check\.sh main' \
    "release runbook benchmark artifact gate"
  require_pattern docs/release-runbook.md \
    'scripts/release-dry-run\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release runbook dry run orchestration"
  require_pattern docs/release-runbook.md \
    'evidence-file release-evidence/vX\.Y\.Z\.md' \
    "release runbook dry run evidence archive"
  require_pattern docs/release-runbook.md \
    'scripts/release-tag-preflight\.sh --repo sleticalboy/CodeInsight-mcp vX\.Y\.Z main' \
    "release runbook tag preflight gate"
  require_pattern docs/release-runbook.md \
    'GitHub Release already exists for the tag' \
    "release runbook release conflict gate"
  require_pattern docs/release-runbook.md \
    'target tag must match' \
    "release runbook metadata consistency gate"
  require_pattern docs/release-runbook.md \
    '`metadata_cargo`, `metadata_install`, and `metadata_changelog`' \
    "release runbook metadata summary output"
  require_pattern docs/release-runbook.md \
    'scripts/archive-release-evidence\.sh --repo sleticalboy/CodeInsight-mcp --json-output release-evidence/vX\.Y\.Z\.json vX\.Y\.Z main' \
    "release runbook evidence archive"
  require_pattern docs/release-runbook.md \
    'release-evidence/vX\.Y\.Z\.md' \
    "release runbook evidence archive path"
  require_pattern docs/release-runbook.md \
    'verify-pretag-ci' \
    "release runbook tag pretag workflow gate"
  require_pattern docs/release-runbook.md \
    'codeinsight-mcp-first-call' \
    "release runbook MCP first-call artifact"
  require_pattern docs/release-runbook.md \
    'Cancelled, failed, or in-progress' \
    "release runbook cancelled CI evidence guard"
  require_pattern docs/release-runbook.md \
    'runs cannot satisfy release evidence' \
    "release runbook non-success CI evidence guard"
  require_pattern docs/release-runbook.md \
    'successful run for the tag target SHA' \
    "release runbook tag SHA successful CI binding"
  require_pattern docs/release-runbook.md \
    'release pretag evidence' \
    "release runbook pretag evidence summary"
  require_pattern docs/release-runbook.md \
    'artifact_gate_benchmark' \
    "release runbook pretag benchmark evidence summary"
  require_pattern docs/release-runbook.md \
    'benchmark_context_pack_first' \
    "release runbook pretag benchmark routing metric"
  require_pattern docs/release-runbook.md \
    'benchmark_line_reduction' \
    "release runbook pretag benchmark line reduction metric"
  require_pattern docs/release-runbook.md \
    'handoff summary and release notes draft read benchmark metrics' \
    "release runbook handoff benchmark metrics flow"
  require_pattern docs/release-runbook.md \
    '\[CodeInsight self adoption report\]\(adoption-report-codeinsight\.md\)' \
    "release runbook adoption report handoff"
  require_pattern docs/release-runbook.md \
    'report fields from that JSON' \
    "release runbook adoption report JSON flow"
  require_pattern docs/release-runbook.md \
    'artifact_gate_mcp_first_call' \
    "release runbook pretag MCP first-call evidence summary"
  require_pattern scripts/release-pretag-check.sh \
    'gh run watch "\$RUN_ID".*--exit-status' \
    "release pretag CI watch"
  require_pattern scripts/release-pretag-check.sh \
    '\-\-head-sha SHA' \
    "release pretag head SHA option"
  require_pattern scripts/release-pretag-check.sh \
    'head_sha: \$RESOLVED_HEAD_SHA' \
    "release pretag resolved head SHA output"
  require_pattern scripts/release-pretag-check.sh \
    'artifact_gate_agent_route: passed' \
    "release pretag agent-route gate summary"
  require_pattern scripts/release-pretag-check.sh \
    'benchmark_context_pack_first:' \
    "release pretag benchmark routing metric"
  require_pattern scripts/release-pretag-check.sh \
    'benchmark_line_reduction:' \
    "release pretag benchmark line reduction metric"
  require_pattern scripts/release-pretag-check.sh \
    'benchmark_guardrail_failures:' \
    "release pretag benchmark guardrail metric"
  require_pattern scripts/release-pretag-check.sh \
    'artifact_gate_mcp_first_call: passed' \
    "release pretag MCP first-call gate summary"
  require_pattern scripts/release-dry-run.sh \
    'benchmark_summary:' \
    "release dry run benchmark summary checklist"
  require_pattern scripts/release-evidence-summary.sh \
    'metrics' \
    "release evidence benchmark metrics JSON"
  require_pattern scripts/release-handoff-summary.sh \
    'Benchmark routing:' \
    "release handoff benchmark routing"
  require_pattern scripts/release-notes-draft.sh \
    'Benchmark Evidence' \
    "release notes benchmark evidence section"
  require_pattern scripts/release-pretag-check.sh \
    'BENCHMARK_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag artifact smoke hook"
  require_pattern scripts/release-pretag-check.sh \
    'CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag context-pack quality artifact smoke hook"
  require_pattern scripts/release-pretag-check.sh \
    'MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag MCP first-call artifact smoke hook"
  require_pattern scripts/release-tag-preflight.sh \
    'RELEASE_METADATA_SUMMARY_SCRIPT' \
    "release tag preflight metadata summary hook"
  require_pattern scripts/release-evidence-summary.sh \
    'RELEASE_METADATA_SUMMARY_SCRIPT' \
    "release evidence metadata summary hook"
  require_pattern scripts/release-evidence-summary.sh \
    'context_pack_quality_artifact_url' \
    "release evidence context-pack quality artifact URL"
  require_pattern scripts/release-evidence-summary.sh \
    'mcp_first_call_artifact_url' \
    "release evidence MCP first-call artifact URL"
  require_pattern scripts/release-evidence-summary.sh \
    'adoption_report_line_reduction' \
    "release evidence adoption report reduction output"
  require_pattern scripts/release-handoff-summary.sh \
    'Adoption report routed first-read' \
    "release handoff adoption report metrics"
  require_pattern scripts/release-notes-draft.sh \
    'Adoption Report Evidence' \
    "release notes adoption report evidence section"
  require_pattern scripts/archive-release-evidence.sh \
    'release-evidence/\$TAG_NAME\.md' \
    "archive release evidence default path"
  require_pattern scripts/archive-release-evidence.sh \
    'output file already exists' \
    "archive release evidence overwrite guard"
  require_pattern scripts/release-evidence-summary-artifact-smoke.sh \
    'context_pack_quality_artifact_url' \
    "release evidence summary artifact smoke context-pack quality URL"
  require_pattern scripts/release-evidence-summary-artifact-smoke.sh \
    'mcp_first_call_artifact_url' \
    "release evidence summary artifact smoke MCP first-call URL"
  require_pattern scripts/release-evidence-summary-artifact-smoke.sh \
    '\-\-run-id "\$RUN_ID"' \
    "release evidence summary artifact smoke fixed run ID validation"
  require_pattern scripts/release-evidence-summary-artifact-smoke.sh \
    '\-\-head-sha "\$head_sha"' \
    "release evidence summary artifact smoke fixed run SHA validation"
  require_pattern scripts/release-dry-run.sh \
    'CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT' \
    "release dry run context-pack quality artifact hook"
  require_pattern scripts/release-dry-run.sh \
    'CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT' \
    "release dry run MCP first-call artifact hook"
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
    'compact benchmark summary' \
    "maintainer compact benchmark summary guidance"
  require_pattern docs/maintainer-checklist.md \
    'release-handoff-summary\.sh' \
    "maintainer release handoff benchmark metrics guidance"
  require_pattern docs/maintainer-checklist.md \
    'update-release-status\.sh' \
    "maintainer release status adoption report guidance"
  require_pattern docs/release-readiness.md \
    'benchmark routing, line-reduction lines' \
    "release readiness handoff benchmark metrics guidance"
  require_pattern docs/release-readiness.md \
    'adoption report routed first-read' \
    "release readiness adoption report status guidance"
  require_pattern docs/release-readiness.md \
    'MCP first-call contract booleans' \
    "release readiness adoption report contract guidance"
  require_pattern docs/maintainer-checklist.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "maintainer benchmark artifact smoke command"
  require_pattern docs/maintenance-commands.md \
    'summary uploaded by `benchmark-subset-smoke`' \
    "maintenance benchmark summary artifact validation"
  require_pattern docs/status.md \
    'MCP first-call onboarding' \
    "status MCP first-call evidence artifact"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    'gh run download "\$RUN_ID"' \
    "benchmark artifact gh download command"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    '\-\-latest-success BRANCH' \
    "benchmark artifact latest successful run option"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    'benchmark-report-smoke\.sh' \
    "benchmark artifact report validation"
  require_pattern scripts/benchmark-artifact-smoke.sh \
    'benchmark-summary-text\.sh' \
    "benchmark artifact summary JSON validation"
  require_pattern scripts/release-evidence-summary.sh \
    'benchmark_summary:' \
    "release evidence benchmark summary output"
  require_pattern scripts/release-evidence-summary.sh \
    'Benchmark summary:' \
    "release notes benchmark summary output"
  require_pattern scripts/agent-router-demo.sh \
    'reading_plan_steps' \
    "agent-router reading plan output"
  require_pattern scripts/agent-router-demo.sh \
    'first_next_action' \
    "agent-router next action output"
  require_pattern scripts/agent-router-demo.sh \
    'first_reading_question' \
    "agent-router first reading question output"
  require_pattern scripts/agent-router-demo.sh \
    'reading_plan_reason' \
    "agent-router reading-plan reason output"
  require_pattern scripts/agent-router-demo.sh \
    'selection_reason' \
    "agent-router selection reason output"
  require_pattern scripts/agent-router-demo.sh \
    'impact_breakdown\.call_related_files' \
    "agent-router impact breakdown output"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_number_gt_zero "\$context_json" '\''\.reading_plan \| length'\''' \
    "agent-router reading plan assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.next_action'\''' \
    "agent-router next action assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.question'\''' \
    "agent-router first reading question assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.reason'\''' \
    "agent-router reading-plan reason assertion"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route_execution_plan_steps' \
    "MCP stdio execution plan steps output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route_first_execution_action' \
    "MCP stdio first execution action output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route_suggested_tool_executed' \
    "MCP stdio execution-plan suggested tool execution output"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First reading question' \
    "agent-route step summary first reading question"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'first_reading_question' \
    "agent-route artifact first reading question output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route execution_plan suggested file_outline did not return entrypoint symbol' \
    "MCP stdio execution-plan suggested tool assertion"
  require_pattern docs/mcp-client-smoke.md \
    '`agent_route\.execution_plan\[\]` includes `read_selected_context`' \
    "MCP client smoke execution plan start"
  require_pattern docs/mcp-client-smoke.md \
    '`use_current_reading_step_suggested_tool`, `use_continuation_if_needed`, and' \
    "MCP client smoke execution plan middle"
  require_pattern docs/mcp-client-smoke.md \
    '`review_impact_before_edits`' \
    "MCP client smoke execution plan impact checkpoint"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_execution_plan_steps: 4' \
    "MCP client smoke execution plan output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_suggested_tool_executed: true' \
    "MCP client smoke execution-plan suggested tool output"
  require_pattern docs/mcp-client-smoke.md \
    'suggested tool is a usable MCP call' \
    "MCP client smoke execution-plan suggested tool contract"
  require_pattern docs/mcp-client-smoke.md \
    'not permission' \
    "MCP client smoke suggested tool ordering boundary"
  require_pattern docs/mcp-client-smoke.md \
    'continuation follow-ups remain gated behind selected-context reading' \
    "MCP client smoke continuation ordering boundary"
  require_pattern docs/mcp-client-smoke.md \
    '`review_impact_before_edits` remains the pre-edit planning checkpoint' \
    "MCP client smoke impact checkpoint boundary"
  require_pattern docs/mcp-client-smoke.md \
    'smoke proves protocol usability; it does not change the first-read ordering' \
    "MCP client smoke protocol usability boundary"
  require_pattern docs/mcp-client-config.md \
    '^## First Agent Route Call$' \
    "MCP client config first agent route section"
  require_pattern docs/mcp-client-config.md \
    '"name": "agent_route"' \
    "MCP client config first agent_route tools call"
  require_pattern docs/mcp-client-config.md \
    '"token_budget": 6000' \
    "MCP client config first agent_route token budget"
  require_pattern docs/mcp-client-config.md \
    'Offer `execution_plan\[\]\.suggested_tool` only after the selected file has' \
    "MCP client config suggested tool ordering"
  require_pattern docs/mcp-client-config.md \
    'checks that `agent_route\.execution_plan\[\]\.suggested_tool` executes through MCP' \
    "MCP client config suggested tool execution smoke"
  require_pattern docs/mcp-client-config.md \
    'scripts/mcp-first-call-smoke\.sh' \
    "MCP client config first-call smoke command"
  require_pattern docs/mcp-client-config.md \
    '`route_tools`, `selected_files`, `execution_plan_actions`' \
    "MCP client config first-call JSON fields"
  require_pattern docs/mcp-client-config.md \
    '`reading_plan\[\]`, suggested-tool handoff checks' \
    "MCP client config first-call handoff fields"
  require_pattern docs/mcp-client-config.md \
    'Expected summary shape:' \
    "MCP client config first-call summary example"
  require_pattern docs/mcp-client-config.md \
    '"selected_files": \["src/main\.ts", "src/auth\.ts"\]' \
    "MCP client config first-call selected files example"
  require_pattern docs/mcp-client-config.md \
    '"use_current_reading_step_suggested_tool"' \
    "MCP client config first-call execution action example"
  require_pattern docs/mcp-client-config.md \
    '"tool": "file_outline"' \
    "MCP client config first-call suggested tool example"
  require_pattern docs/mcp-client-config.md \
    'Expected first-call signals:' \
    "MCP client config first-call signals"
  require_pattern docs/mcp-client-config.md \
    '\| `route\[\]` \| Includes `index_project`, `project_overview`, `context_pack`, and `impact_analysis`\.' \
    "MCP client config route signal"
  require_pattern docs/mcp-client-config.md \
    '\| `execution_plan\[\]` \| Starts with `read_selected_context`, then gates deeper tools and continuation\.' \
    "MCP client config execution plan signal"
  require_pattern docs/client-workflow.md \
    'The first call is healthy when the response has:' \
    "client workflow first-call health checklist"
  require_pattern docs/client-workflow.md \
    '`execution_plan\[0\]\.action` set to `read_selected_context`' \
    "client workflow first execution action health check"
  require_pattern scripts/two-minute-demo.sh \
    'Problem: AI agents waste the first read' \
    "two-minute demo problem statement"
  require_pattern scripts/two-minute-demo.sh \
    'agent-route' \
    "two-minute demo agent-route command"
  require_pattern scripts/two-minute-demo.sh \
    'CodeInsight agent_route demo' \
    "two-minute demo agent_route heading"
  require_pattern scripts/two-minute-demo.sh \
    'project_overview found' \
    "two-minute demo overview talk track"
  require_pattern scripts/two-minute-demo.sh \
    'context_pack selected' \
    "two-minute demo context-pack talk track"
  require_pattern scripts/two-minute-demo.sh \
    'first_execution_suggested_tool' \
    "two-minute demo execution-plan suggested tool metric"
  require_pattern scripts/two-minute-demo.sh \
    'first_reading_question' \
    "two-minute demo first reading question metric"
  require_pattern scripts/two-minute-demo.sh \
    'first_reading_file' \
    "two-minute demo first reading file metric"
  require_pattern scripts/two-minute-demo.sh \
    'reading_order_contract' \
    "two-minute demo reading order contract metric"
  require_pattern scripts/two-minute-demo.sh \
    'suggested_tool_handoff_contract' \
    "two-minute demo suggested tool handoff contract metric"
  require_pattern scripts/two-minute-demo.sh \
    'continuation_timing_contract' \
    "two-minute demo continuation timing contract metric"
  require_pattern scripts/two-minute-demo.sh \
    'execution_plan\[0\]\.files follows reading_plan\[\] order' \
    "two-minute demo reading order contract talk track"
  require_pattern scripts/two-minute-demo.sh \
    'execution_plan\[1\] points to the current reading step' \
    "two-minute demo suggested tool handoff talk track"
  require_pattern scripts/two-minute-demo.sh \
    'offer it only after the selected file has been read' \
    "two-minute demo suggested tool gating talk track"
  require_pattern scripts/two-minute-demo.sh \
    '\[Evidence summary\]' \
    "two-minute demo evidence summary section"
  require_pattern scripts/two-minute-demo.sh \
    'agent_route selected \$\{selected_lines\}/\$\{total_lines\} source lines' \
    "two-minute demo line-reduction evidence summary"
  require_pattern scripts/two-minute-demo.sh \
    'impact_analysis reports' \
    "two-minute demo impact-analysis talk track"

  echo "docs benchmark smoke passed"
}

main "$@"
