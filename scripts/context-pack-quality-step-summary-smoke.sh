#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

fail() {
  echo "context-pack quality step summary smoke failed: $*" >&2
  exit 1
}

require_literal() {
  local file="$1"
  local literal="$2"
  local description="$3"

  if ! grep -Fq -- "$literal" "$file"; then
    fail "$file is missing $description"
  fi
}

main() {
  local summary_json
  local step_summary

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  summary_json="$TEMP_DIR/summary.json"
  step_summary="$TEMP_DIR/step-summary.md"

  cat >"$summary_json" <<'EOF'
{
  "status": "pass",
  "scenarios_passed": 3,
  "scenarios": [
    {
      "name": "budget_continuation",
      "status": "pass",
      "metrics": {
        "candidate_files": 80,
        "selected_files": 12,
        "omitted_candidates": 8,
        "first_next_action": "inspect_seed_file",
        "first_suggested_tool": "file_outline",
        "first_reading_question": "Where does the feature route start?",
        "first_reading_reason": "Read this step to answer: Where does the feature route start? If deeper evidence is needed, call file_outline.",
        "first_selection_reason": "Matched explicit seed file | ranked first",
        "first_reason_actionable": true,
        "continuation_status": "omitted_candidates_available"
      }
    },
    {
      "name": "minimum_budget",
      "status": "pass",
      "metrics": {
        "requested_token_budget": 20,
        "applied_token_budget": 500,
        "continuation_status": "minimum_budget_applied"
      }
    },
    {
      "name": "token_exhaustion",
      "status": "pass",
      "metrics": {
        "selected_files": 12,
        "truncated": true,
        "continuation_status": "token_budget_exhausted"
      }
    }
  ],
  "question_checks_passed": 2,
  "question_checks": [
    {
      "name": "seed_file_auth_question",
      "status": "pass",
      "file": "src/auth.ts",
      "next_action": "inspect_seed_file",
      "focus": "Start with seed file authentication and session boundaries.",
      "question": "Where are authentication decisions, credentials, or session boundaries handled here?",
      "suggested_tool": "file_outline"
    },
    {
      "name": "semantic_session_cookie_question",
      "status": "pass",
      "file": "src/auth_notes.py",
      "next_action": "review_semantic_matches",
      "focus": "Review semantic matches for authentication, cookie, or session behavior.",
      "question": "Which semantic matches describe authentication, credential, cookie, or session behavior?",
      "suggested_tool": "context_pack"
    }
  ]
}
EOF

  GITHUB_STEP_SUMMARY="$step_summary" \
    "$ROOT_DIR/scripts/context-pack-quality-step-summary.sh" \
    "$summary_json" \
    codeinsight-context-pack-quality \
    "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1/artifacts/2" \
    "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1" >/dev/null

  require_literal "$step_summary" "## Context Pack Quality Smoke" "summary title"
  require_literal "$step_summary" "Scenarios passed: \`3\`" "scenario count"
  require_literal "$step_summary" "Workflow run: [open run](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1)" "run link"
  require_literal "$step_summary" 'Workflow artifact: [`codeinsight-context-pack-quality`](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1/artifacts/2)' "artifact link"
  require_literal "$step_summary" "| Scenario | Status | Key Metrics |" "scenario table"
  require_literal "$step_summary" '| `budget_continuation` | `pass` | `candidate_files=80`' "budget continuation row"
  require_literal "$step_summary" '`first_reading_question=Where does the feature route start?`' "first reading question metric"
  require_literal "$step_summary" '`first_reading_reason=Read this step to answer: Where does the feature route start? If deeper evidence is needed, call file_outline.`' "first reading reason metric"
  require_literal "$step_summary" '`first_selection_reason=Matched explicit seed file \\| ranked first`' "escaped selection reason metric"
  require_literal "$step_summary" '`first_reason_actionable=true`' "reading-plan reason metric"
  require_literal "$step_summary" '`continuation_status=token_budget_exhausted`' "token exhaustion metric"
  require_literal "$step_summary" "Question checks passed: \`2\`" "question checks count"
  require_literal "$step_summary" "| Check | Next Action | File | Focus | Question | Suggested Tool |" "question checks table"
  require_literal "$step_summary" '| `seed_file_auth_question` | `inspect_seed_file` | `src/auth.ts` | Start with seed file authentication and session boundaries. | Where are authentication decisions, credentials, or session boundaries handled here? | `file_outline` |' "seed question row"
  require_literal "$step_summary" '| `semantic_session_cookie_question` | `review_semantic_matches` | `src/auth_notes.py` | Review semantic matches for authentication, cookie, or session behavior. | Which semantic matches describe authentication, credential, cookie, or session behavior? | `context_pack` |' "semantic question row"

  echo "context-pack quality step summary smoke passed"
}

main "$@"
