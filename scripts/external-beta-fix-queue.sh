#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: scripts/external-beta-fix-queue.sh COHORT_SUMMARY_JSON [options]

Turns an External Beta cohort JSON summary into a maintainer fix queue.

Options:
  --output PATH       Markdown output path. Default: /tmp/codeinsight-external-beta-fix-queue.md.
  --json PATH         JSON output path. Default: <output-dir>/external-beta-fix-queue.json.
  --max-items N       Maximum queue items to emit. Default: all actionable items.
  --check             Fail when there is no actionable fix item.
  -h, --help          Show this help text.
EOF
}

fail() {
  echo "external beta fix queue failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

main() {
  local cohort_json=""
  local output="/tmp/codeinsight-external-beta-fix-queue.md"
  local json_output=""
  local max_items=""
  local check="false"

  while [ "$#" -gt 0 ]; do
    case "$1" in
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        output="$2"
        shift 2
        ;;
      --json)
        [ "$#" -ge 2 ] || fail "--json requires a path"
        json_output="$2"
        shift 2
        ;;
      --max-items)
        [ "$#" -ge 2 ] || fail "--max-items requires a number"
        max_items="$2"
        shift 2
        ;;
      --check)
        check="true"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        fail "unknown argument: $1"
        ;;
      *)
        [ -z "$cohort_json" ] || fail "only one cohort JSON input is supported"
        cohort_json="$1"
        shift
        ;;
    esac
  done

  [ -n "$cohort_json" ] || fail "cohort JSON input is required"
  [ -f "$cohort_json" ] || fail "cohort JSON does not exist: $cohort_json"
  case "$max_items" in
    ''|*[!0-9]*) [ -z "$max_items" ] || fail "--max-items must be a positive integer" ;;
  esac
  if [ -n "$max_items" ] && [ "$max_items" -le 0 ]; then
    fail "--max-items must be greater than zero"
  fi

  require_command ruby
  mkdir -p "$(dirname "$output")"
  if [ -z "$json_output" ]; then
    json_output="$(dirname "$output")/external-beta-fix-queue.json"
  fi
  mkdir -p "$(dirname "$json_output")"

  ruby -rjson -rtime - "$cohort_json" "$output" "$json_output" "$max_items" "$check" <<'RUBY'
cohort_path = ARGV.fetch(0)
output = ARGV.fetch(1)
json_output = ARGV.fetch(2)
max_items_arg = ARGV.fetch(3)
check = ARGV.fetch(4) == "true"
max_items = max_items_arg.empty? ? nil : Integer(max_items_arg)

def fail_with(message)
  warn "external beta fix queue failed: #{message}"
  exit 1
end

begin
  cohort = JSON.parse(File.read(cohort_path))
rescue JSON::ParserError => error
  fail_with("#{cohort_path} is not valid JSON: #{error.message}")
end

unless cohort.key?("reports") && cohort.key?("classification_counts")
  fail_with("#{cohort_path} does not look like an external beta cohort summary")
end

reports = cohort.fetch("reports")
quality_failure_paths = cohort.dig("quality_gate", "failures").to_a.map { |failure| failure["path"].to_s }

def action_for(priority)
  case priority
  when "needs_triage" then "Reclassify the report and record the maintainer decision."
  when "workflow_friction" then "Fix the trial workflow, docs, or issue filing path before routing changes."
  when "route_quality_below_threshold" then "Improve the route or evidence quality until it passes the cohort gate."
  when "route_miss" then "Reproduce the task, add a routing regression, then adjust selection signals."
  when "overtrust_risk" then "Tighten wording so users see the route as a first-read hint, not a certainty claim."
  when "route_near_miss" then "Tune the small ranking gap only after higher priority items are clean."
  else "No maintainer action required."
  end
end

def rationale_for(priority)
  case priority
  when "needs_triage" then "Unclassified reports cannot become public evidence."
  when "workflow_friction" then "Blocked trial setup prevents external users from producing usable evidence."
  when "route_quality_below_threshold" then "Low confidence routes should remain feedback, not success evidence."
  when "route_miss" then "The first selected file is wrong for a real task."
  when "overtrust_risk" then "Public copy may overstate best-effort static routing."
  when "route_near_miss" then "Useful route, but the first read still needs a small ranking improvement."
  else ""
  end
end

def repo_label(report)
  if report["private_repo"] == true
    "private/redacted"
  elsif report["repository_url"].to_s.length.positive?
    report["repository_url"].to_s
  else
    report["repository"].to_s
  end
end

priority_rules = [
  ["needs_triage", ->(report) { report["outcome"] == "needs_triage" }],
  ["workflow_friction", ->(report) { report["outcome"] == "workflow_friction" }],
  ["route_quality_below_threshold", ->(report) { quality_failure_paths.include?(report["path"].to_s) }],
  ["route_miss", ->(report) { report["outcome"] == "route_miss" }],
  ["overtrust_risk", ->(report) { report["outcome"] == "overtrust_risk" }],
  ["route_near_miss", ->(report) { report["outcome"] == "route_near_miss" }]
]

items = []
priority_rules.each do |priority, predicate|
  reports.select { |report| predicate.call(report) }.each do |report|
    next if items.any? { |item| item["summary_path"] == report["path"] && item["priority"] == priority }

    items << {
      "rank" => items.length + 1,
      "priority" => priority,
      "outcome" => report["outcome"].to_s,
      "repository" => repo_label(report),
      "task" => report["task"].to_s,
      "first_file" => report["first_file"].to_s,
      "route_quality_score" => report["route_quality_score"],
      "route_quality_level" => report["route_quality_level"].to_s,
      "read_less_ratio" => report["read_less_ratio"].to_s,
      "summary_path" => report["path"].to_s,
      "issue_body" => report["issue_body"].to_s,
      "maintainer_triage" => report["maintainer_triage"].to_s,
      "recommended_action" => action_for(priority),
      "rationale" => rationale_for(priority)
    }
  end
end

items = items.first(max_items) if max_items
status = items.empty? ? "empty" : "actionable"

queue = {
  "status" => status,
  "generated_at" => Time.now.utc.iso8601,
  "source" => cohort_path,
  "cohort_status" => cohort["status"].to_s,
  "cohort_next_action" => cohort["next_action"].to_s,
  "item_count" => items.length,
  "items" => items,
  "artifacts" => {
    "markdown" => output,
    "json" => json_output
  }
}

def cell(value)
  value.to_s.gsub("|", "\\|").gsub("\n", " ")
end

rows =
  if items.empty?
    ["| - | - | - | - | - | - | - |"]
  else
    items.map do |item|
      quality = item["route_quality_score"].nil? ? "n/a" : "#{item["route_quality_level"]} / #{item["route_quality_score"]}"
      "| #{item["rank"]} | `#{cell(item["priority"])}` | #{cell(item["repository"])} | #{cell(item["task"])} | `#{cell(item["first_file"])}` | `#{cell(quality)}` | #{cell(item["recommended_action"])} |"
    end
  end

details =
  if items.empty?
    "- No actionable External Beta fixes. The cohort is ready for the next public handoff step."
  else
    items.map do |item|
      [
        "### #{item["rank"]}. #{item["priority"]}",
        "",
        "- Task: #{item["task"]}",
        "- First file: `#{item["first_file"]}`",
        "- Summary: `#{item["summary_path"]}`",
        "- Maintainer triage: `#{item["maintainer_triage"].empty? ? "-" : item["maintainer_triage"]}`",
        "- Rationale: #{item["rationale"]}",
        "- Recommended action: #{item["recommended_action"]}"
      ].join("\n")
    end.join("\n\n")
  end

File.write(output, <<~MARKDOWN)
  # External Beta Fix Queue

  This queue is generated from `#{cohort_path}`. It turns cohort outcomes into
  maintainer work items for the local-first AI-agent first-read routing workflow.

  ## Summary

  - Status: `#{status}`
  - Cohort status: `#{cohort["status"]}`
  - Cohort next action: `#{cohort["next_action"]}`
  - Items: `#{items.length}`

  ## Queue

  | Rank | Priority | Repository | Task | First file | Route quality | Recommended action |
  | ---: | --- | --- | --- | --- | --- | --- |
  #{rows.join("\n")}

  ## Details

  #{details}
MARKDOWN

File.write(json_output, JSON.pretty_generate(queue) + "\n")

if check && items.empty?
  fail_with("no actionable External Beta fix items")
end

puts "external beta fix queue written to #{output}"
puts "queue_json: #{json_output}"
puts "status: #{status}"
puts "items: #{items.length}"
RUBY
}

main "$@"
