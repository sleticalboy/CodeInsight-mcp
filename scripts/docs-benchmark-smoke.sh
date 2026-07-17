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
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "smoke reading plan guardrail"
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
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "large reading plan guardrail"
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
    'Pick the validation that matches your goal:' \
    "README validation chooser"
  require_pattern README.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/mcp-first-call-smoke\.sh' \
    "README MCP first-call smoke command"
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
  require_pattern docs/maintenance-commands.md \
    'scripts/benchmark-artifact-smoke\.sh <ci-run-id>' \
    "maintenance benchmark artifact download"
  require_pattern docs/maintenance-commands.md \
    'scripts/context-pack-quality-artifact-smoke\.sh <ci-run-id>' \
    "maintenance context-pack quality artifact download"
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
    'selected files, execution plan, suggested tool, impact status, and saved artifacts' \
    "maintenance MCP first-call artifact scope"
  require_pattern docs/maintenance-commands.md \
    'scripts/mcp-first-call-step-summary-smoke\.sh' \
    "maintenance MCP first-call step summary smoke"
  require_pattern docs/maintenance-commands.md \
    '\| First MCP call Actions summary changed \| `scripts/mcp-first-call-step-summary-smoke\.sh` \|' \
    "maintenance MCP first-call step summary chooser"
  require_pattern docs/maintenance-commands.md \
    'Actions Summary section for selected files, execution plan, suggested tool, impact status, and artifact link' \
    "maintenance MCP first-call step summary scope"
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
  require_pattern docs/maintainer-checklist.md \
    'context-pack quality smoke' \
    "maintainer context-pack quality smoke"
  require_pattern docs/maintainer-checklist.md \
    'context-pack-quality-smoke` job summary' \
    "maintainer context-pack quality CI summary guidance"
  require_pattern docs/maintainer-checklist.md \
    'scripts/context-pack-quality-artifact-smoke\.sh <ci-run-id>' \
    "maintainer context-pack quality artifact smoke command"
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
    'BENCHMARK_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag artifact smoke hook"
  require_pattern scripts/release-pretag-check.sh \
    'CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT' \
    "release pretag context-pack quality artifact smoke hook"
  require_pattern scripts/release-tag-preflight.sh \
    'RELEASE_METADATA_SUMMARY_SCRIPT' \
    "release tag preflight metadata summary hook"
  require_pattern scripts/release-evidence-summary.sh \
    'RELEASE_METADATA_SUMMARY_SCRIPT' \
    "release evidence metadata summary hook"
  require_pattern scripts/release-evidence-summary.sh \
    'context_pack_quality_artifact_url' \
    "release evidence context-pack quality artifact URL"
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
    '\-\-run-id "\$RUN_ID"' \
    "release evidence summary artifact smoke fixed run ID validation"
  require_pattern scripts/release-evidence-summary-artifact-smoke.sh \
    '\-\-head-sha "\$head_sha"' \
    "release evidence summary artifact smoke fixed run SHA validation"
  require_pattern scripts/release-dry-run.sh \
    'CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT' \
    "release dry run context-pack quality artifact hook"
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
