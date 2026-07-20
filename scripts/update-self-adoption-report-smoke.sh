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
  echo "update self adoption report smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/src"
  echo 'fn main() {}' >"$TEMP_DIR/repo/src/main.rs"

  cat >"$TEMP_DIR/adoption-report" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$1"
shift
output_dir=""
archive=""
task=""
token_budget=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --archive)
      archive="$2"
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
    --print-snippet)
      shift
      ;;
    --no-force-index)
      shift
      ;;
    *)
      echo "unexpected argument: $1" >&2
      exit 2
      ;;
  esac
done

[ -n "$output_dir" ] || exit 3
[ -n "$archive" ] || exit 4
mkdir -p "$output_dir" "$(dirname "$archive")"
summary_path="$output_dir/summary.json"
manifest_path="$output_dir/manifest.json"

cat >"$summary_path" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "$task",
  "output_dir": "$output_dir",
  "local_evidence": {
    "status": "pass",
    "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
    "metrics": {
      "indexed_files": 3,
      "symbols": 17,
      "index_errors": 0,
      "entrypoints": 1,
      "total_lines": 1200,
      "selected_lines": 80,
      "source_lines_avoided": 1120,
      "line_reduction": "93.3%",
      "read_less_ratio": "15.0x",
      "selected_files": 2,
      "selected_ranges": 2,
      "estimated_tokens": 900,
      "reading_plan_steps": 2,
      "selected_seed_count": 1,
      "seed_strategy": "auto_entrypoint",
      "first_seed_source": "overview_entrypoint",
      "first_seed_value": "src/main.rs",
      "companion_entrypoint": "",
      "first_file": "src/main.rs",
      "first_reading_focus": "Start with Rust entrypoint wiring.",
      "first_reading_question": "What entrypoints define the main flow here?",
      "first_next_action": "inspect_seed_file",
      "first_suggested_tool": "file_outline",
      "risk_level": "medium",
      "impacted_files": 4
    }
  },
  "mcp_first_call": {
    "status": "pass",
    "server": "codeinsight",
    "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
    "execution_plan_reads_in_reading_plan_order": true,
    "current_reading_step_matches_reading_plan": true,
    "first_execution_instruction_has_read_less": true,
    "current_step_suggested_tool_matches_reading_plan": true,
    "continuation_after_selected_context": true,
    "suggested_tool_executed": true,
    "impact_status": "complete",
    "first_context_file": "src/main.rs",
    "first_reading_file": "src/main.rs",
    "suggested_tool": {
      "tool": "file_outline",
      "arguments": {
        "path": "$repo_root/src/main.rs"
      }
    }
  },
  "first_read_gating": {
    "suggested_tool_after_selected_context": true,
    "continuation_after_selected_context": true,
    "impact_review_before_edits": true
  }
}
JSON

cat >"$manifest_path" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "output_dir": "$output_dir",
  "archive": "$archive",
  "files": [
    "adoption-evidence.md",
    "summary.json",
    "issue-template.md",
    "local-repo-evidence.md",
    "local-repo-evidence.json",
    "agent-route.json",
    "mcp-first-call.json",
    "local-repo-evidence.out",
    "local-repo-evidence.err",
    "mcp-first-call.out",
    "mcp-first-call.err",
    "artifact-write.err",
    "manifest.json"
  ]
}
JSON

for file in adoption-evidence.md issue-template.md local-repo-evidence.md local-repo-evidence.json agent-route.json mcp-first-call.json local-repo-evidence.out local-repo-evidence.err mcp-first-call.out mcp-first-call.err artifact-write.err; do
  : >"$output_dir/$file"
done
tar -czf "$archive" -C "$output_dir" adoption-evidence.md summary.json issue-template.md local-repo-evidence.md local-repo-evidence.json agent-route.json mcp-first-call.json local-repo-evidence.out local-repo-evidence.err mcp-first-call.out mcp-first-call.err artifact-write.err manifest.json
echo "stub adoption report written to $output_dir"
EOF
  chmod +x "$TEMP_DIR/adoption-report"

  "$ROOT_DIR/scripts/update-self-adoption-report.sh" \
    --root "$TEMP_DIR/repo" \
    --task "understand the main application entrypoint" \
    --token-budget 6000 \
    --output "$TEMP_DIR/adoption-report-codeinsight.md" \
    --output-dir "$TEMP_DIR/report-output" \
    --archive "$TEMP_DIR/codeinsight-self-adoption-report.tar.gz" \
    --report-script "$TEMP_DIR/adoption-report" \
    --refreshed-on 2026-07-18 \
    >"$TEMP_DIR/output.log"

  grep -Fq "updated self adoption report: $TEMP_DIR/adoption-report-codeinsight.md" "$TEMP_DIR/output.log" ||
    fail "missing update output"
  grep -Fq -- "- Refreshed on: \`2026-07-18\`" "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing refreshed date"
  grep -Fq '| CodeInsight routed first-read | `80` source lines |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing routed first-read metric"
  grep -Fq '| First-read reduction | `93.3%` |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing reduction metric"
  grep -Fq '| Source lines avoided | `1120` |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing source lines avoided metric"
  grep -Fq '| Read less | `15.0x` |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing read-less metric"
  grep -Fq '| First reading focus | Start with Rust entrypoint wiring. |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing first reading focus"
  grep -Fq '| Current reading step mirrors reading plan | `true` |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing current reading step mirror metric"
  grep -Fq '| First execution instruction carries read-less evidence | `true` |' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing read-less instruction contract metric"
  grep -Fq -- '- First reading focus: Start with Rust entrypoint wiring.' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing generated snippet first reading focus"
  grep -Fq -- '- Source lines avoided: `1120`' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing generated snippet source lines avoided"
  grep -Fq -- '- Read less: `15.0x`' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing generated snippet read less"
  grep -Fq -- '- MCP first-call contract: reading_order=`true`, current_reading_step=`true`, read_less_instruction=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing generated snippet read-less instruction contract"
  grep -Fq 'The generated manifest reported `status: pass` and listed the same 13 files' "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing manifest evidence"
  grep -Fq "file_outline\` with an absolute \`$TEMP_DIR/repo/src/main.rs\` path" "$TEMP_DIR/adoption-report-codeinsight.md" ||
    fail "missing suggested tool path"

  "$ROOT_DIR/scripts/update-self-adoption-report.sh" \
    --root "$TEMP_DIR/repo" \
    --task "understand the main application entrypoint" \
    --token-budget 6000 \
    --output "$TEMP_DIR/adoption-report-codeinsight.md" \
    --output-dir "$TEMP_DIR/report-output" \
    --archive "$TEMP_DIR/codeinsight-self-adoption-report.tar.gz" \
    --report-script "$TEMP_DIR/adoption-report" \
    --check \
    >"$TEMP_DIR/check-output.log"

  grep -Fq "self adoption report is up to date" "$TEMP_DIR/check-output.log" ||
    fail "check mode did not pass"

  echo "stale" >>"$TEMP_DIR/adoption-report-codeinsight.md"
  if "$ROOT_DIR/scripts/update-self-adoption-report.sh" \
    --root "$TEMP_DIR/repo" \
    --task "understand the main application entrypoint" \
    --token-budget 6000 \
    --output "$TEMP_DIR/adoption-report-codeinsight.md" \
    --output-dir "$TEMP_DIR/report-output" \
    --archive "$TEMP_DIR/codeinsight-self-adoption-report.tar.gz" \
    --report-script "$TEMP_DIR/adoption-report" \
    --check \
    >"$TEMP_DIR/stale-output.log" 2>"$TEMP_DIR/stale-error.log"; then
    fail "check mode passed for stale output"
  fi
  grep -Fq "self adoption report is out of date" "$TEMP_DIR/stale-error.log" ||
    fail "check mode did not report stale output"

  echo "update self adoption report smoke passed"
}

main "$@"
