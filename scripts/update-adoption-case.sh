#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CASE_NAME="${CODEINSIGHT_ADOPTION_CASE_NAME:-}"
REPO_URL="${CODEINSIGHT_ADOPTION_CASE_REPO_URL:-}"
REPO_ROOT="${CODEINSIGHT_ADOPTION_CASE_ROOT:-}"
COMMIT_REF="${CODEINSIGHT_ADOPTION_CASE_COMMIT:-}"
WORK_DIR="${CODEINSIGHT_ADOPTION_CASE_WORK_DIR:-}"
OUTPUT_FILE="${CODEINSIGHT_ADOPTION_CASE_OUTPUT:-}"
TASK="${CODEINSIGHT_ADOPTION_CASE_TASK:-}"
TOKEN_BUDGET="${CODEINSIGHT_ADOPTION_CASE_TOKEN_BUDGET:-6000}"
COMPARISON_SCRIPT="${CODEINSIGHT_ADOPTION_COMPARISON_SCRIPT:-$ROOT_DIR/scripts/adoption-comparison.sh}"
COMPARISON_OUTPUT_DIR="${CODEINSIGHT_ADOPTION_CASE_OUTPUT_DIR:-}"
CASE_TITLE=""
CASE_SUBJECT=""
CASE_WRAPPER_NOTE=""

usage() {
  cat <<'EOF'
usage: scripts/update-adoption-case.sh CASE [options]

Refreshes a checked-in adoption case from a live adoption-comparison run.

Supported cases:
  express

Options:
  --root PATH           Existing repository checkout. Skips clone.
  --repo-url URL        Repository URL. Defaults to the case repository.
  --commit REF          Commit, branch, or tag to check out after clone.
  --work-dir PATH       Temporary work directory. Default: /tmp/codeinsight-adoption-case-<case>.
  --output PATH         Output Markdown path. Default: docs/adoption-case-<case>.md.
  --output-dir PATH     adoption-comparison output directory. Default: <work-dir>/evidence.
  --task TEXT           Task passed to adoption-comparison.
  --token-budget N      Token budget passed to adoption-comparison. Default: 6000.
  --comparison-script PATH
                        adoption-comparison-compatible script to execute.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_ADOPTION_CASE_NAME
  CODEINSIGHT_ADOPTION_CASE_REPO_URL
  CODEINSIGHT_ADOPTION_CASE_ROOT
  CODEINSIGHT_ADOPTION_CASE_COMMIT
  CODEINSIGHT_ADOPTION_CASE_WORK_DIR
  CODEINSIGHT_ADOPTION_CASE_OUTPUT
  CODEINSIGHT_ADOPTION_CASE_TASK
  CODEINSIGHT_ADOPTION_CASE_TOKEN_BUDGET
  CODEINSIGHT_ADOPTION_COMPARISON_SCRIPT
  CODEINSIGHT_ADOPTION_CASE_OUTPUT_DIR
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "update adoption case failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      express)
        if [ -n "$CASE_NAME" ] && [ "$CASE_NAME" != "$1" ]; then
          fail "case specified more than once"
        fi
        CASE_NAME="$1"
        shift
        ;;
      --root)
        [ "$#" -ge 2 ] || fail "--root requires a path"
        REPO_ROOT="$2"
        shift 2
        ;;
      --repo-url)
        [ "$#" -ge 2 ] || fail "--repo-url requires a URL"
        REPO_URL="$2"
        shift 2
        ;;
      --commit)
        [ "$#" -ge 2 ] || fail "--commit requires a ref"
        COMMIT_REF="$2"
        shift 2
        ;;
      --work-dir)
        [ "$#" -ge 2 ] || fail "--work-dir requires a path"
        WORK_DIR="$2"
        shift 2
        ;;
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT_FILE="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        COMPARISON_OUTPUT_DIR="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail "--task requires text"
        TASK="$2"
        shift 2
        ;;
      --token-budget)
        [ "$#" -ge 2 ] || fail "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --comparison-script)
        [ "$#" -ge 2 ] || fail "--comparison-script requires a path"
        COMPARISON_SCRIPT="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        fail "unknown argument: $1"
        ;;
      *)
        fail "unknown adoption case: $1"
        ;;
    esac
  done
}

configure_case() {
  if [ -z "$CASE_NAME" ]; then
    fail "missing adoption case name"
  fi

  case "$CASE_NAME" in
    express)
      CASE_TITLE="Express Adoption Comparison"
      CASE_SUBJECT="Express"
      REPO_URL="${REPO_URL:-https://github.com/expressjs/express.git}"
      WORK_DIR="${WORK_DIR:-/tmp/codeinsight-adoption-case-express}"
      OUTPUT_FILE="${OUTPUT_FILE:-$ROOT_DIR/docs/adoption-case-express.md}"
      TASK="${TASK:-understand express application routing behavior}"
      CASE_WRAPPER_NOTE='The legacy wrapper `scripts/update-adoption-case-express.sh` delegates to the same command.'
      ;;
    *)
      fail "unsupported adoption case: $CASE_NAME"
      ;;
  esac
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

prepare_repo() {
  if [ -n "$REPO_ROOT" ]; then
    [ -d "$REPO_ROOT" ] || fail "repository root does not exist: $REPO_ROOT"
    REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
    return
  fi

  rm -rf "$WORK_DIR/repo"
  mkdir -p "$WORK_DIR"

  if [ -n "$COMMIT_REF" ]; then
    git -c http.version=HTTP/1.1 clone --quiet "$REPO_URL" "$WORK_DIR/repo"
    git -C "$WORK_DIR/repo" checkout --quiet "$COMMIT_REF"
  else
    git -c http.version=HTTP/1.1 clone --quiet --depth 1 "$REPO_URL" "$WORK_DIR/repo"
  fi

  REPO_ROOT="$WORK_DIR/repo"
}

git_value() {
  local query="$1"
  local fallback="$2"

  if git -C "$REPO_ROOT" $query >/dev/null 2>&1; then
    git -C "$REPO_ROOT" $query
  else
    printf "%s" "$fallback"
  fi
}

write_case_doc() {
  local summary_json="$1"
  local target="$2"
  local commit_full commit_short
  local blind_lines routed_lines avoided reduction read_less selected_files selected_ranges tokens
  local seed_strategy first_seed_source first_seed_value companion first_file first_question first_tool risk impacted

  commit_full="$(git_value "rev-parse HEAD" "local")"
  commit_short="$(git_value "rev-parse --short HEAD" "local")"
  blind_lines="$(json_value "$summary_json" '.metrics.blind_first_read_lines')"
  routed_lines="$(json_value "$summary_json" '.metrics.routed_first_read_lines')"
  avoided="$(json_value "$summary_json" '.metrics.source_lines_avoided')"
  reduction="$(json_value "$summary_json" '.metrics.line_reduction')"
  read_less="$(json_value "$summary_json" '.metrics.read_less_ratio')"
  selected_files="$(json_value "$summary_json" '.metrics.selected_files')"
  selected_ranges="$(json_value "$summary_json" '.metrics.selected_ranges')"
  tokens="$(json_value "$summary_json" '.metrics.estimated_tokens')"
  seed_strategy="$(json_value "$summary_json" '.metrics.seed_strategy')"
  first_seed_source="$(json_value "$summary_json" '.metrics.first_seed_source')"
  first_seed_value="$(json_value "$summary_json" '.metrics.first_seed_value')"
  companion="$(json_value "$summary_json" '(.metrics.companion_entrypoint // "") as $value | if $value == "" then "-" else $value end')"
  first_file="$(json_value "$summary_json" '.metrics.first_file')"
  first_question="$(json_value "$summary_json" '.metrics.first_reading_question')"
  first_tool="$(json_value "$summary_json" '.metrics.first_suggested_tool')"
  risk="$(json_value "$summary_json" '.metrics.risk_level')"
  impacted="$(json_value "$summary_json" '.metrics.impacted_files')"

  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOF
# $CASE_TITLE

This is a reproducible adoption case for CodeInsight as a local-first AI-agent
code context router. It uses $CASE_SUBJECT as a public repository and
compares a blind first read of all indexed source lines with CodeInsight's
routed first-read context.

This is adoption evidence, not a controlled performance benchmark. The goal is
to show what an AI coding agent can read first before opening files broadly.

## Snapshot

- Repository: \`$REPO_URL\`
- Commit: \`$commit_full\`
- Short commit: \`$commit_short\`
- Task: \`$TASK\`
- Token budget: \`$TOKEN_BUDGET\`
- Route: \`$(json_value "$summary_json" '.route_tools | join(" -> ")')\`
- Generated with: \`scripts/update-adoption-case.sh $CASE_NAME\`

## Result

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | \`$blind_lines\` source lines |
| CodeInsight routed first-read | \`$routed_lines\` source lines |
| Source lines avoided | \`$avoided\` |
| First-read reduction | \`$reduction\` |
| Read less | \`$read_less\` |
| Selected files | \`$selected_files\` |
| Selected ranges | \`$selected_ranges\` |
| Estimated tokens | \`$tokens\` |
| Impacted files | \`$impacted\` |

## First-Read Route

| Field | Value |
| --- | --- |
| Seed strategy | \`$seed_strategy\` |
| First seed source | \`$first_seed_source\` |
| First seed value | \`$first_seed_value\` |
| Companion entrypoint | \`$companion\` |
| First selected file | \`$first_file\` |
| First suggested tool | \`$first_tool\` |
| Impact risk | \`$risk\` |

First reading question:

\`\`\`text
$first_question
\`\`\`

## Reproduce

Refresh this checked-in snapshot:

\`\`\`bash
scripts/update-adoption-case.sh $CASE_NAME
\`\`\`

Recreate this exact snapshot:

\`\`\`bash
scripts/update-adoption-case.sh $CASE_NAME --commit $commit_full
\`\`\`

$CASE_WRAPPER_NOTE

Generate a fresh comparison against the current $CASE_SUBJECT default branch:

\`\`\`bash
rm -rf /tmp/codeinsight-case-$CASE_NAME
git clone --depth 1 $REPO_URL /tmp/codeinsight-case-$CASE_NAME
scripts/adoption-comparison.sh /tmp/codeinsight-case-$CASE_NAME \\
  --task "$TASK" \\
  --output-dir /tmp/codeinsight-adoption-case-$CASE_NAME
\`\`\`

For exact snapshot comparison, check out the commit listed above before running
the script.

Artifacts written by the command:

- \`$COMPARISON_OUTPUT_DIR/adoption-comparison.md\`
- \`$COMPARISON_OUTPUT_DIR/summary.json\`
- \`$COMPARISON_OUTPUT_DIR/local-repo-evidence.json\`
- \`$COMPARISON_OUTPUT_DIR/agent-route.json\`

EOF
}

main() {
  parse_args "$@"
  require_command git
  require_command jq
  configure_case

  if [ ! -x "$COMPARISON_SCRIPT" ]; then
    fail "comparison script is not executable: $COMPARISON_SCRIPT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail "--token-budget must be greater than zero"
  fi

  prepare_repo
  COMPARISON_OUTPUT_DIR="${COMPARISON_OUTPUT_DIR:-$WORK_DIR/evidence}"
  rm -rf "$COMPARISON_OUTPUT_DIR"
  mkdir -p "$COMPARISON_OUTPUT_DIR"

  "$COMPARISON_SCRIPT" "$REPO_ROOT" \
    --task "$TASK" \
    --token-budget "$TOKEN_BUDGET" \
    --output-dir "$COMPARISON_OUTPUT_DIR"

  jq -e \
    '.status == "pass"
      and (.metrics.blind_first_read_lines | type == "number")
      and (.metrics.routed_first_read_lines | type == "number")
      and (.metrics.read_less_ratio | type == "string" and length > 0)' \
    "$COMPARISON_OUTPUT_DIR/summary.json" >/dev/null ||
    fail "comparison summary does not match the adoption case contract"

  write_case_doc "$COMPARISON_OUTPUT_DIR/summary.json" "$OUTPUT_FILE"

  echo "updated adoption case: $OUTPUT_FILE"
  echo "repository: $REPO_ROOT"
  echo "commit: $(git_value "rev-parse HEAD" "local")"
  echo "summary: $COMPARISON_OUTPUT_DIR/summary.json"
}

main "$@"
