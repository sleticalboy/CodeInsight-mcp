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
  echo "update adoption case smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/lib"
  echo 'module.exports = require("./lib/express")' >"$TEMP_DIR/repo/index.js"
  echo 'exports.application = function application() {}' >"$TEMP_DIR/repo/lib/express.js"
  git -C "$TEMP_DIR/repo" init --quiet
  git -C "$TEMP_DIR/repo" config user.email "codeinsight@example.invalid"
  git -C "$TEMP_DIR/repo" config user.name "CodeInsight Smoke"
  git -C "$TEMP_DIR/repo" add .
  git -C "$TEMP_DIR/repo" commit --quiet -m "fixture"
  commit="$(git -C "$TEMP_DIR/repo" rev-parse HEAD)"

  cat >"$TEMP_DIR/adoption-comparison" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$1"
shift
output_dir=""
task=""
token_budget=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --task)
      task="$2"
      shift 2
      ;;
    --token-budget)
      token_budget="$2"
      shift 2
      ;;
    *)
      echo "unexpected argument: $1" >&2
      exit 2
      ;;
  esac
done

[ -n "$output_dir" ] || exit 3
mkdir -p "$output_dir"
cat >"$output_dir/summary.json" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "$task",
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "metrics": {
    "blind_first_read_lines": 21478,
    "routed_first_read_lines": 232,
    "source_lines_avoided": 21246,
    "line_reduction": "98.9%",
    "read_less_ratio": "92.6x",
    "selected_files": 6,
    "selected_ranges": 7,
    "estimated_tokens": 1589,
    "seed_strategy": "auto_task_match",
    "first_seed_source": "task_match",
    "first_seed_value": "lib/express.js",
    "companion_entrypoint": "",
    "first_file": "lib/express.js",
    "first_reading_question": "What entrypoints, exported symbols, or setup code define the main flow here?",
    "first_suggested_tool": "file_outline",
    "risk_level": "high",
    "impacted_files": 27
  }
}
JSON
touch "$output_dir/adoption-comparison.md" "$output_dir/local-repo-evidence.json" "$output_dir/agent-route.json"
echo "stub comparison written to $output_dir"
EOF
  chmod +x "$TEMP_DIR/adoption-comparison"

  "$ROOT_DIR/scripts/update-adoption-case.sh" \
    express \
    --root "$TEMP_DIR/repo" \
    --repo-url "https://github.com/expressjs/express.git" \
    --comparison-script "$TEMP_DIR/adoption-comparison" \
    --output "$TEMP_DIR/adoption-case-express.md" \
    --work-dir "$TEMP_DIR/work" \
    >"$TEMP_DIR/output.log"

  grep -Fq "updated adoption case: $TEMP_DIR/adoption-case-express.md" "$TEMP_DIR/output.log" ||
    fail "missing update output"
  grep -Fq "commit: $commit" "$TEMP_DIR/output.log" ||
    fail "missing commit output"
  grep -Fq -- "- Commit: \`$commit\`" "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing commit in generated doc"
  grep -Fq -- "- Generated with: \`scripts/update-adoption-case.sh express\`" "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing generator line"
  grep -Fq '| Blind first-read baseline | `21478` source lines |' "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing baseline metric"
  grep -Fq '| Read less | `92.6x` |' "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing read-less metric"
  grep -Fq 'scripts/update-adoption-case.sh express' "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing refresh command"
  grep -Fq "scripts/update-adoption-case.sh express --commit $commit" "$TEMP_DIR/adoption-case-express.md" ||
    fail "missing exact snapshot command"

  "$ROOT_DIR/scripts/update-adoption-case-express.sh" \
    --root "$TEMP_DIR/repo" \
    --repo-url "https://github.com/expressjs/express.git" \
    --comparison-script "$TEMP_DIR/adoption-comparison" \
    --output "$TEMP_DIR/adoption-case-express-wrapper.md" \
    --work-dir "$TEMP_DIR/wrapper-work" \
    >"$TEMP_DIR/wrapper-output.log"

  grep -Fq "updated adoption case: $TEMP_DIR/adoption-case-express-wrapper.md" "$TEMP_DIR/wrapper-output.log" ||
    fail "wrapper did not delegate to generic updater"

  "$ROOT_DIR/scripts/update-adoption-case.sh" \
    gin \
    --root "$TEMP_DIR/repo" \
    --repo-url "https://github.com/gin-gonic/gin.git" \
    --comparison-script "$TEMP_DIR/adoption-comparison" \
    --output "$TEMP_DIR/adoption-case-gin.md" \
    --work-dir "$TEMP_DIR/gin-work" \
    >"$TEMP_DIR/gin-output.log"

  grep -Fq "updated adoption case: $TEMP_DIR/adoption-case-gin.md" "$TEMP_DIR/gin-output.log" ||
    fail "gin case did not use generic updater"
  grep -Fq '# Gin Adoption Comparison' "$TEMP_DIR/adoption-case-gin.md" ||
    fail "missing gin case title"
  grep -Fq -- "- Repository: \`https://github.com/gin-gonic/gin.git\`" "$TEMP_DIR/adoption-case-gin.md" ||
    fail "missing gin repository"
  grep -Fq -- "- Generated with: \`scripts/update-adoption-case.sh gin\`" "$TEMP_DIR/adoption-case-gin.md" ||
    fail "missing gin generator line"

  "$ROOT_DIR/scripts/update-adoption-cases.sh" \
    --output "$TEMP_DIR/adoption-cases.md" \
    "$TEMP_DIR/adoption-case-express.md" \
    "$TEMP_DIR/adoption-case-gin.md" \
    >"$TEMP_DIR/summary-output.log"

  grep -Fq "updated adoption cases summary: $TEMP_DIR/adoption-cases.md" "$TEMP_DIR/summary-output.log" ||
    fail "summary updater did not report output path"
  grep -Fq 'Blind first-read baseline: `42,956` source lines' "$TEMP_DIR/adoption-cases.md" ||
    fail "summary updater did not aggregate baselines"
  grep -Fq 'Aggregate read-less ratio: `92.6x`' "$TEMP_DIR/adoption-cases.md" ||
    fail "summary updater did not aggregate read-less ratio"
  "$ROOT_DIR/scripts/update-adoption-cases.sh" \
    --check \
    --output "$TEMP_DIR/adoption-cases.md" \
    "$TEMP_DIR/adoption-case-express.md" \
    "$TEMP_DIR/adoption-case-gin.md" \
    >"$TEMP_DIR/summary-check-output.log"
  grep -Fq "adoption cases summary is up to date" "$TEMP_DIR/summary-check-output.log" ||
    fail "summary updater check mode did not pass"

  echo "update adoption case smoke passed"
}

main "$@"
