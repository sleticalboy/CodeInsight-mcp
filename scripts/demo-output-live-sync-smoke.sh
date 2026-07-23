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
  echo "demo output live sync smoke failed: $*" >&2
  exit 1
}

main() {
  if ! command -v ruby >/dev/null 2>&1; then
    fail "missing required command: ruby"
  fi

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  CODEINSIGHT_DEMO_ROOT="$ROOT_DIR" \
    "$ROOT_DIR/scripts/two-minute-demo.sh" >"$TEMP_DIR/two-minute-demo.out"

  ruby - "$ROOT_DIR" "$TEMP_DIR/two-minute-demo.out" <<'RUBY'
root = ARGV.fetch(0)
output = File.read(ARGV.fetch(1))

documents = {
  "README.md" => File.read(File.join(root, "README.md")),
  "docs/demo-output.md" => File.read(File.join(root, "docs", "demo-output.md")),
  "docs/demo-script.md" => File.read(File.join(root, "docs", "demo-script.md")),
  "docs/public-demo-one-pager.md" => File.read(File.join(root, "docs", "public-demo-one-pager.md"))
}

def metric(output, label)
  match = output.match(/^\s+#{Regexp.escape(label)}: (.+)$/)
  unless match
    warn "live demo output is missing #{label}"
    exit 1
  end
  match[1].strip
end

def with_commas(value)
  value.to_s.gsub(",", "").reverse.gsub(/(\d{3})(?=\d)/, '\\1,').reverse
end

values = {
  indexed_files: metric(output, "indexed_files"),
  symbols: metric(output, "symbols"),
  total_lines: metric(output, "total_lines"),
  entrypoints: metric(output, "entrypoints"),
  recommended_next_tools: metric(output, "recommended_next_tools"),
  selected_files: metric(output, "selected_files"),
  selected_ranges: metric(output, "selected_ranges"),
  reading_plan_steps: metric(output, "reading_plan_steps"),
  routing_read_less: metric(output, "routing_decision_read_less"),
  routing_continuation: metric(output, "routing_decision_continuation"),
  blind_lines: metric(output, "blind_first_read_lines"),
  routed_lines: metric(output, "routed_first_read_lines"),
  avoided_lines: metric(output, "source_lines_avoided"),
  line_reduction: metric(output, "line_reduction"),
  read_less_ratio: metric(output, "read_less_ratio"),
  continuation_next_action: metric(output, "continuation_next_action"),
  first_omitted_candidate: metric(output, "first_omitted_candidate"),
  impacted_files: metric(output, "impacted_files"),
  paths: metric(output, "paths"),
  suggested_checks: metric(output, "suggested_checks")
}

selection_evidence = output.lines.find { |line| line.start_with?("Selection evidence: ") }
unless selection_evidence
  warn "live demo output is missing Selection evidence summary line"
  exit 1
end
selection_evidence = selection_evidence.strip

def require_includes(documents, file, expected, description)
  return if documents.fetch(file).include?(expected)

  warn "#{file} is out of sync with live two-minute demo output: #{description}"
  warn "expected to include: #{expected}"
  exit 1
end

["docs/demo-output.md"].each do |file|
  require_includes(documents, file, "indexed_files: #{values[:indexed_files]}", "indexed file count")
  require_includes(documents, file, "symbols: #{values[:symbols]}", "symbol count")
  require_includes(documents, file, "total_lines: #{values[:total_lines]}", "total line count")
  require_includes(documents, file, "selected_files: #{values[:selected_files]}", "selected file count")
  require_includes(documents, file, "selected_ranges: #{values[:selected_ranges]}", "selected range count")
  require_includes(documents, file, "reading_plan_steps: #{values[:reading_plan_steps]}", "reading-plan step count")
  require_includes(documents, file, "routing_decision_read_less: #{values[:routing_read_less]}", "routing read-less metric")
  require_includes(documents, file, "routing_decision_continuation: #{values[:routing_continuation]}", "routing continuation status")
  require_includes(documents, file, "blind_first_read_lines: #{values[:blind_lines]}", "blind first-read baseline")
  require_includes(documents, file, "routed_first_read_lines: #{values[:routed_lines]}", "routed first-read lines")
  require_includes(documents, file, "source_lines_avoided: #{values[:avoided_lines]}", "avoided source lines")
  require_includes(documents, file, "line_reduction: #{values[:line_reduction]}", "line reduction")
  require_includes(documents, file, "read_less_ratio: #{values[:read_less_ratio]}", "read-less ratio")
  require_includes(documents, file, "continuation_next_action: #{values[:continuation_next_action]}", "continuation next action")
  require_includes(documents, file, "first_omitted_candidate: #{values[:first_omitted_candidate]}", "omitted candidate")
  require_includes(documents, file, "impacted_files: #{values[:impacted_files]}", "impacted file count")
  require_includes(documents, file, "paths: #{values[:paths]}", "impact path count")
  require_includes(documents, file, "suggested_checks: #{values[:suggested_checks]}", "suggested check count")
end

summary = {
  selected: values[:routed_lines],
  total: values[:blind_lines],
  avoided: values[:avoided_lines],
  reduction: values[:line_reduction],
  ratio: values[:read_less_ratio],
  files: values[:selected_files],
  continuation: values[:routing_continuation],
  impacted: values[:impacted_files]
}

readme_line = "selecting #{summary[:selected]} of #{with_commas(summary[:total])} source lines, avoiding #{with_commas(summary[:avoided])} source lines before broad reading for a #{summary[:reduction]} reduction and #{summary[:ratio]} read-less ratio"
require_includes(documents, "README.md", readme_line, "README current demo summary")

["docs/demo-script.md", "docs/public-demo-one-pager.md"].each do |file|
  require_includes(documents, file, "routing_decision_read_less: #{values[:routing_read_less]}", "routing read-less metric")
  require_includes(documents, file, "routing_decision_continuation: #{values[:routing_continuation]}", "routing continuation status")
  require_includes(documents, file, "impacted_files: #{values[:impacted_files]}", "impacted file count")
  require_includes(documents, file, "Routing decision: seed=task_match:src/tools.rs, first_file=src/tools.rs, rank=1, tool=file_outline, continuation=#{summary[:continuation]}, impact=complete.", "routing decision summary")
  require_includes(documents, file, "Before edits, impact_analysis reports high risk across #{summary[:impacted]} impacted files.", "impact summary")
end

["docs/demo-output.md", "docs/demo-script.md"].each do |file|
  require_includes(documents, file, "Blind first-read baseline: #{summary[:total]} source lines.", "baseline evidence line")
  require_includes(documents, file, "Routed first-read: #{summary[:selected]} source lines across #{summary[:files]} files.", "routed evidence line")
  require_includes(documents, file, "Read less: avoided #{summary[:avoided]} source lines, #{summary[:ratio]} less text before follow-up tools.", "read-less evidence line")
  require_includes(documents, file, "agent_route selected #{summary[:selected]}/#{summary[:total]} source lines (#{summary[:reduction]} reduction) across #{summary[:files]} files.", "line reduction evidence line")
  require_includes(documents, file, selection_evidence, "selection evidence line")
  require_includes(documents, file, "project_overview found #{values[:entrypoints]} entrypoints and #{values[:recommended_next_tools]} recommended next tools.", "overview talk-track line")
end
RUBY

  echo "demo output live sync smoke passed"
}

main "$@"
