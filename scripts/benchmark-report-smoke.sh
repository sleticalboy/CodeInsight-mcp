#!/usr/bin/env bash
set -euo pipefail

REPORT_FILE="${1:-}"
EXPECTED_PROFILE="${2:-}"
EXPECTED_REPOS="${3:-}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/benchmark-report-smoke.sh REPORT_FILE [PROFILE] [REPO[,REPO...]]

Validates the generated benchmark Markdown report structure. Use PROFILE and
REPO filters for subset artifacts, for example:

  scripts/benchmark-report-smoke.sh /tmp/codeinsight-benchmark-subset.md smoke p-limit
EOF
}

fail() {
  echo "benchmark report smoke failed: $*" >&2
  exit 1
}

require_pattern() {
  local pattern="$1"
  local description="$2"

  if ! grep -Eq "$pattern" "$REPORT_FILE"; then
    fail "$REPORT_FILE is missing $description"
  fi
}

require_literal() {
  local literal="$1"
  local description="$2"

  if ! grep -Fq -- "$literal" "$REPORT_FILE"; then
    fail "$REPORT_FILE is missing $description"
  fi
}

require_repo_section() {
  local repo="$1"

  require_pattern "^## $repo$" "$repo detail section"
  require_pattern "^\\| $repo \\|" "$repo summary row"
}

main() {
  local repo

  if [ -z "$REPORT_FILE" ] || [ "$REPORT_FILE" = "-h" ] || [ "$REPORT_FILE" = "--help" ]; then
    usage
    exit 2
  fi
  if [ ! -s "$REPORT_FILE" ]; then
    fail "$REPORT_FILE does not exist or is empty"
  fi

  require_pattern '^# CodeInsight v0\.1 .+ Benchmark$' "benchmark title"
  require_literal "This is a benchmark fixture report, not a controlled performance benchmark." "fixture disclaimer"
  require_pattern '^- Profile: `[^`]+`$' "profile metadata"
  require_pattern '^- Context pack mode: .+6000 token budget$' "context-pack budget metadata"
  require_pattern '^## Summary$' "summary section"
  require_pattern '^\| Repository \| Focus \| Commit \| Files \| Lines \| Symbols \|' "summary header"
  require_pattern '^\| [^|]+ \| [^|]+ \| `[0-9a-f]{7,}` \|' "at least one summary row"
  require_literal "| pass |" "passing budget status"
  require_literal "| \`context_pack\` |" "context_pack recommendation evidence"
  require_pattern '^## Key Results$' "key results section"
  require_pattern '^- Repositories benchmarked: [0-9]+ \(`[^`]+` subset\)\.$' "repository count key result"
  require_pattern '^- Agent routing: `context_pack` was the first recommended tool for [0-9]+/[0-9]+ repositories\.$' "agent routing key result"
  require_pattern '^- Context compression: selected [0-9]+ of [0-9]+ source lines \([0-9.]+% reduction\) across [0-9]+ files and [0-9]+ ranges\.$' "context compression key result"
  require_pattern '^- Token budget: [0-9]+ estimated tokens total, [0-9]+ average tokens per repository, with a 6000 token budget per context pack\.$' "token budget key result"
  require_pattern '^- Guardrails: [0-9]+ context, [0-9]+ symbol, [0-9]+ call target, and [0-9]+ call edge failures\.$' "guardrail key result"
  require_pattern '^## Details$' "details section"
  require_literal "Recommended next tools:" "recommended tools detail section"
  require_literal "Context pack files:" "context-pack files detail section"
  require_literal "Context reading plan:" "context reading-plan detail section"
  require_literal "| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |" "context reading-plan rank and focus columns"
  require_literal "- Context continuation next action:" "continuation next action detail"
  require_literal "- First omitted candidate:" "omitted candidate detail"
  require_pattern '^\| `[^`]+` \| [0-9]+ \| [^|]+ \| What ' "context reading-plan focus text"
  require_pattern "What (entrypoints, exported symbols, or setup code define the main flow here|startup entrypoint or initialization sequence creates the requested flow)\\?" "context reading-plan question text"
  require_literal "Read this step to answer:" "actionable reading-plan reason"
  require_literal "Context pack guardrails:" "context-pack guardrail section"
  require_literal "| \`first_recommended_tool\` | context_pack | context_pack | pass |" "context_pack guardrail pass"
  require_pattern '^\| `selected_files` \| >= [0-9]+ \| [0-9]+ \| pass \|$' "selected files guardrail pass"
  require_pattern '^\| `reading_plan_steps` \| >= [0-9]+ \| [0-9]+ \| pass \|$' "reading plan guardrail pass"
  require_pattern '^\| `first_reading_focus` \| present \| .+ \| pass \|$' "first reading focus guardrail pass"
  require_pattern '^\| `first_reading_question` \| present \| .+ \| pass \|$' "first reading question guardrail pass"
  require_pattern '^\| `first_reading_reason` \| present \| .+ \| pass \|$' "first reading reason guardrail pass"
  require_pattern '^\| `first_selection_rank` \| >= 1 \| [0-9]+ \| pass \|$' "first selection rank guardrail pass"
  require_pattern '^\| `first_selection_reason` \| present \| .+ \| pass \|$' "first selection reason guardrail pass"
  require_pattern '^\| `line_reduction` \| >= [0-9]+% \| [0-9.]+% \| pass \|$' "line reduction guardrail pass"

  if [ -n "$EXPECTED_PROFILE" ]; then
    require_literal "- Profile: \`$EXPECTED_PROFILE\`" "$EXPECTED_PROFILE profile metadata"
  fi
  if [ -n "$EXPECTED_REPOS" ]; then
    require_pattern '^- Repository subset: `[^`]+`$' "repository subset metadata"
    require_literal "- Repository subset: \`$EXPECTED_REPOS\`" "$EXPECTED_REPOS repository subset metadata"
    IFS="," read -r -a repos <<<"$EXPECTED_REPOS"
    for repo in "${repos[@]}"; do
      require_repo_section "$repo"
    done
  fi

  echo "benchmark report smoke passed"
}

main "$@"
