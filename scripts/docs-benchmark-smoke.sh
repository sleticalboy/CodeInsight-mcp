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

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$ROOT_DIR/$file" >/dev/null; then
    echo "$file is missing ${description}" >&2
    echo "query: $query" >&2
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

require_readme_benchmark_summary_sync() {
  ruby - "$ROOT_DIR" <<'RUBY'
root = ARGV.fetch(0)
readme = File.read(File.join(root, "README.md"))

def fetch_context_compression(root, report)
  content = File.read(File.join(root, report))
  match = content.match(/^- Context compression: selected ([0-9,]+) of ([0-9,]+) source lines \(([0-9.]+)% reduction\)/)
  unless match
    warn "#{report} is missing context compression key result"
    exit 1
  end

  [match[1], match[2], match[3]]
end

def with_commas(value)
  value.to_s.gsub(",", "").reverse.gsub(/(\d{3})(?=\d)/, '\\1,').reverse
end

smoke_selected, smoke_total, smoke_reduction =
  fetch_context_compression(root, "docs/benchmark-v0.1.md")
large_selected, large_total, large_reduction =
  fetch_context_compression(root, "docs/benchmark-large.md")

expected_smoke = "- Smoke repositories route `context_pack` first for 4/4 repositories and\n" \
  "  select #{with_commas(smoke_selected)} of #{with_commas(smoke_total)} source lines, a #{smoke_reduction}% aggregate line reduction."
expected_large = "- Large repositories route `context_pack` first for 4/4 repositories and\n" \
  "  select #{with_commas(large_selected)} of #{with_commas(large_total)} source lines, a #{large_reduction}% aggregate line reduction."

unless readme.include?(expected_smoke)
  warn "README smoke benchmark summary is out of sync with docs/benchmark-v0.1.md"
  warn "expected:"
  warn expected_smoke
  exit 1
end

unless readme.include?(expected_large)
  warn "README large benchmark summary is out of sync with docs/benchmark-large.md"
  warn "expected:"
  warn expected_large
  exit 1
end
RUBY
}

require_self_adoption_summary_sync() {
  ruby - "$ROOT_DIR" <<'RUBY'
root = ARGV.fetch(0)
readme = File.read(File.join(root, "README.md"))
release_commands = File.read(File.join(root, "docs", "release-commands.md"))
self_report = File.read(File.join(root, "docs", "adoption-report-codeinsight.md"))

def fetch_metric(content, label)
  match = content.match(/^\| #{Regexp.escape(label)} \| `([^`]+)`(?: source lines)? \|$/)
  unless match
    warn "docs/adoption-report-codeinsight.md is missing #{label}"
    exit 1
  end
  match[1]
end

def with_commas(value)
  value.to_s.gsub(",", "").reverse.gsub(/(\d{3})(?=\d)/, '\\1,').reverse
end

selected = fetch_metric(self_report, "CodeInsight routed first-read")
total = fetch_metric(self_report, "Blind first-read baseline")
avoided = fetch_metric(self_report, "Source lines avoided")
reduction = fetch_metric(self_report, "First-read reduction")
read_less = fetch_metric(self_report, "Read less")

expected_readme = "- The CodeInsight self adoption report packages a full tar.gz handoff and\n" \
  "  routes the entrypoint task to #{with_commas(selected)} of #{with_commas(total)} source lines, avoiding #{with_commas(avoided)}\n" \
  "  source lines before broad reading for a #{reduction} reduction and #{read_less} read-less\n" \
  "  ratio, with 7 type-relation edges surfaced through the `base_type` graph"
unless readme.include?(expected_readme)
  warn "README self adoption summary is out of sync with docs/adoption-report-codeinsight.md"
  warn "expected:"
  warn expected_readme
  exit 1
end

expected_release_metric = "`/tmp/codeinsight-self-adoption-report.tar.gz` archive path, `#{selected}/#{total}`\n" \
  "routed first-read metric"
unless release_commands.include?(expected_release_metric)
  warn "docs/release-commands.md self adoption metric is out of sync"
  warn "expected:"
  warn expected_release_metric
  exit 1
end
RUBY
}

require_public_route_quality_summary_sync() {
  ruby - "$ROOT_DIR" <<'RUBY'
require "json"

root = ARGV.fetch(0)
summary = JSON.parse(File.read(File.join(root, "docs", "public-task-routing-matrix-summary.json")))
aggregate = summary.fetch("aggregate")

def with_commas(value)
  value.to_s.gsub(",", "").reverse.gsub(/(\d{3})(?=\d)/, '\\1,').reverse
end

def assert_includes(root, file, expected)
  content = File.read(File.join(root, file)).gsub(/\s+/, " ")
  normalized = expected.gsub(/\s+/, " ")
  return if content.include?(normalized)

  warn "#{file} public route-quality summary is out of sync"
  warn "expected:"
  warn expected
  exit 1
end

checks = "#{aggregate.fetch("expectation_count")}/#{aggregate.fetch("task_count")}"
selected = with_commas(aggregate.fetch("total_selected_lines"))
total = with_commas(aggregate.fetch("total_task_source_lines"))
reduction = format("%.2f", aggregate.fetch("line_reduction"))

assert_includes(
  root,
  "README.md",
  "route-quality expectations pass `#{checks}`, selecting #{selected} of #{total} task source lines for a `#{reduction}%` aggregate first-read line reduction"
)
assert_includes(
  root,
  "README.md",
  "passes #{checks} expected first-file checks, selecting #{selected} of #{total} task source lines, a #{reduction}% aggregate first-read line reduction"
)
assert_includes(
  root,
  "README.md",
  "Current public route-quality evidence passes `#{checks}` first-file checks and selects #{selected} of #{total} task source lines."
)
assert_includes(
  root,
  "docs/status.md",
  "passes `#{checks}` expected first-file checks, and selects #{selected} of #{total} task source lines for a #{reduction}% read-less reduction."
)
assert_includes(
  root,
  "docs/mvp-public-readiness.md",
  "Selected lines: `#{selected}` of `#{total}` task source lines."
)
assert_includes(
  root,
  "docs/mvp-public-readiness.md",
  "Aggregate first-read line reduction: `#{reduction}%`."
)
assert_includes(
  root,
  "docs/public-demo-one-pager.md",
  "Express, FastAPI, Flask, Gin, Requests, and Streamlit pass `#{checks}` expected first-file checks."
)
assert_includes(
  root,
  "docs/public-demo-one-pager.md",
  "The public matrix selects `#{selected}` of `#{total}` task source lines for a `#{reduction}%` aggregate first-read line reduction."
)
RUBY
}

main() {
  require_pattern README.md \
    '\[public demo one-pager\]\(docs/public-demo-one-pager\.md\)' \
    "public demo one-pager README link"
  require_pattern README.md \
    '\[two-minute demo script\]\(docs/demo-script\.md\)' \
    "demo script link"
  require_pattern docs/README.md \
    '\[Public demo one-pager\]\(public-demo-one-pager\.md\)' \
    "public demo one-pager docs index link"
  require_pattern docs/public-demo-one-pager.md \
    'agent_route -> selected context -> executable suggested_tool -> impact check' \
    "public demo one-pager workflow"
  require_pattern docs/public-demo-one-pager.md \
    '`routing_decision` gives one compact audit row' \
    "public demo one-pager routing decision framing"
  require_pattern docs/public-demo-one-pager.md \
    'pass `86/86` expected' \
    "public demo one-pager route-quality snapshot"
  require_pattern docs/public-demo-one-pager.md \
    '`99\.42%` aggregate first-read line reduction' \
    "public demo one-pager aggregate line reduction snapshot"
  require_pattern docs/public-demo-one-pager.md \
    'Treat these numbers as first-read routing and token-discipline evidence' \
    "public demo one-pager evidence caveat"
  require_pattern docs/public-demo-one-pager.md \
    'Do not say:' \
    "public demo one-pager guardrail heading"
  require_pattern docs/public-demo-one-pager.md \
    'Compiler-grade static analysis' \
    "public demo one-pager compiler-grade guardrail"
  require_public_route_quality_summary_sync
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
    '\[Task routing matrix\]\(docs/task-routing-matrix\.md\)' \
    "README task routing matrix link"
  require_pattern README.md \
    '\[CodeInsight self adoption report\]\(docs/adoption-report-codeinsight\.md\)' \
    "README self adoption report link"
  require_pattern README.md \
    '2,595 of 675,772 source lines' \
    "README adoption cases aggregate snapshot"
  require_pattern docs/impact-analysis.md \
    'PHP Composer scripts are intentionally broad-only by default' \
    "PHP Composer broad-only suggested-check guidance"
  require_pattern docs/impact-analysis.md \
    'does not match sibling names such as `src/core2\.ts`' \
    "configured suggested-check file filter boundary"
  require_pattern README.md \
    '260\.4x aggregate read-less ratio' \
    "README adoption cases aggregate read-less ratio"
  require_self_adoption_summary_sync
  require_pattern README.md \
    '7 type-relation edges surfaced through the `base_type` graph' \
    "README self adoption report type-relation metric"
  require_pattern README.md \
    '`current_reading_step` mirror and read-less instruction evidence' \
    "README self adoption report read-less instruction evidence"
  require_pattern README.md \
    'includes impact suggested checks' \
    "README MCP wiring impact suggested checks"
  require_pattern README.md \
    'clients can render a pre-edit' \
    "README MCP impact checklist evidence"
  require_pattern README.md \
    'of 76,633 source lines, avoiding 76,092 source lines before broad reading' \
    "README two-minute demo read-less metric"
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
    'scripts/external-beta-trial\.sh /path/to/repo' \
    "README external beta trial command"
  require_pattern README.md \
    'scripts/external-beta-cohort-summary\.sh' \
    "README external beta cohort summary command"
  require_pattern README.md \
    'fix workflow friction' \
    "README external beta cohort workflow friction priority"
  require_pattern README.md \
    'route misses' \
    "README external beta cohort route miss priority"
  require_pattern README.md \
    'codeinsight-external-beta-trial' \
    "README external beta trial output directory"
  require_pattern README.md \
    'can choose `needs_triage`' \
    "README external beta needs triage guidance"
  require_pattern README.md \
    'scripts/adoption-comparison\.sh /path/to/repo' \
    "README adoption comparison command"
  require_pattern README.md \
    'blind-read vs routed-first-read comparison' \
    "README adoption comparison description"
  require_pattern README.md \
    'First selection rank: `<rank>`' \
    "README adoption evidence selection rank snippet"
  require_pattern README.md \
    'Continuation next action: `<next_action>`' \
    "README adoption evidence continuation snippet"
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
    'MCP first-call contract: reading_order=`true`, current_reading_step=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`' \
    "README adoption MCP first-call contract signal"
  require_pattern README.md \
    'First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`' \
    "README adoption first-read gating signal"
  require_pattern README.md \
    'Use `summary\.json` from the same folder' \
    "README adoption summary JSON guidance"
  require_pattern README.md \
    'it includes `first_read_gating`' \
    "README adoption first-read gating JSON guidance"
  require_pattern docs/adoption-checklist.md \
    '\-\-issue-template' \
    "adoption checklist issue template option"
  require_pattern docs/adoption-checklist.md \
    'scripts/external-beta-trial\.sh /path/to/repo' \
    "adoption checklist external beta trial command"
  require_pattern docs/adoption-checklist.md \
    'scripts/external-beta-cohort-summary\.sh' \
    "adoption checklist external beta cohort summary command"
  require_pattern docs/adoption-checklist.md \
    'at least three' \
    "adoption checklist external beta minimum count"
  require_pattern docs/adoption-checklist.md \
    'External Beta reports' \
    "adoption checklist external beta report wording"
  require_pattern docs/adoption-checklist.md \
    'issue-body\.md' \
    "adoption checklist external beta issue body artifact"
  require_pattern docs/adoption-checklist.md \
    'External users can choose `needs_triage`' \
    "adoption checklist external beta needs triage guidance"
  require_pattern docs/adoption-checklist.md \
    'failure category placeholder' \
    "adoption checklist issue template contents"
  require_pattern docs/adoption-checklist.md \
    'scripts/adoption-report\.sh /path/to/repo' \
    "adoption checklist report command"
  require_pattern docs/adoption-checklist.md \
    'include `first_read_gating` signals' \
    "adoption checklist first-read gating summary"
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
    '\[Django adoption case\]\(adoption-case-django\.md\)' \
    "adoption checklist Django case link"
  require_pattern docs/adoption-checklist.md \
    '\[Express adoption case\]\(adoption-case-express\.md\)' \
    "adoption checklist Express case link"
  require_pattern docs/adoption-checklist.md \
    '\[Gin adoption case\]\(adoption-case-gin\.md\)' \
    "adoption checklist Gin case link"
  require_pattern docs/adoption-checklist.md \
    '\[ip2region adoption case\]\(adoption-case-ip2region\.md\)' \
    "adoption checklist ip2region case link"
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
    'scripts/update-adoption-case\.sh django' \
    "adoption checklist Django refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh express' \
    "adoption checklist Express refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh gin' \
    "adoption checklist Gin refresh command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-adoption-case\.sh ip2region' \
    "adoption checklist ip2region refresh command"
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
  require_pattern docs/impact-analysis.md \
    'appends focused commands for those files' \
    "impact analysis focused test command guidance"
  require_pattern docs/impact-analysis.md \
    '`pnpm test -- src/core\.test\.ts`' \
    "impact analysis focused pnpm test example"
  require_pattern docs/impact-analysis.md \
    '`cargo test --locked --test cli`' \
    "impact analysis focused cargo test example"
  require_pattern docs/impact-analysis.md \
    '`mvn -Dtest=TokenNormalizerTest test`' \
    "impact analysis focused maven test example"
  require_pattern docs/impact-analysis.md \
    '`dotnet test --filter FullyQualifiedName~TokenNormalizerTests`' \
    "impact analysis focused dotnet test example"
  require_pattern docs/README.md \
    'Adoption comparison evidence' \
    "docs index adoption comparison validation row"
  require_pattern docs/README.md \
    '\[Adoption cases\]\(adoption-cases\.md\)' \
    "docs index adoption cases summary link"
  require_pattern docs/README.md \
    '\[Task routing matrix\]\(task-routing-matrix\.md\)' \
    "docs index task routing matrix link"
  require_pattern docs/README.md \
    '\[Express\]\(task-routing-expectations/express\.tsv\)' \
    "docs index Express task routing expectation link"
  require_pattern docs/README.md \
    '\[FastAPI\]\(task-routing-expectations/fastapi\.tsv\)' \
    "docs index FastAPI task routing expectation link"
  require_pattern docs/README.md \
    '\[Flask\]\(task-routing-expectations/flask\.tsv\)' \
    "docs index Flask task routing expectation link"
  require_pattern docs/README.md \
    '\[Gin\]\(task-routing-expectations/gin\.tsv\)' \
    "docs index Gin task routing expectation link"
  require_pattern docs/README.md \
    '\[Requests\]\(task-routing-expectations/requests\.tsv\)' \
    "docs index Requests task routing expectation link"
  require_pattern docs/README.md \
    '\[Streamlit\]\(task-routing-expectations/streamlit\.tsv\)' \
    "docs index Streamlit task routing expectation link"
  require_pattern docs/README.md \
    '\[CodeInsight self adoption report\]\(adoption-report-codeinsight\.md\)' \
    "docs index self adoption report link"
  require_pattern docs/README.md \
    'Uploadable adoption report' \
    "docs index uploadable adoption report validation row"
  require_pattern docs/README.md \
    '\[Public Adoption Alpha\]\(public-adoption-alpha\.md\)' \
    "docs index Public Adoption Alpha link"
  require_pattern docs/README.md \
    '\[External Beta trial\]\(external-beta-trial\.md\)' \
    "docs index External Beta trial link"
  require_pattern docs/README.md \
    'External Beta trial pack' \
    "docs index External Beta validation row"
  require_pattern docs/README.md \
    'External Beta cohort summary' \
    "docs index External Beta cohort validation row"
  require_pattern docs/README.md \
    '\[Alpha feedback triage\]\(alpha-feedback-triage\.md\)' \
    "docs index Alpha feedback triage link"
  require_pattern docs/README.md \
    '\[Alpha trial log\]\(alpha-trial-log\.md\)' \
    "docs index Alpha trial log link"
  require_pattern README.md \
    '\[Alpha feedback triage\]\(docs/alpha-feedback-triage\.md\)' \
    "README Alpha feedback triage link"
  require_pattern README.md \
    '\[Alpha trial log\]\(docs/alpha-trial-log\.md\)' \
    "README Alpha trial log link"
  require_pattern README.md \
    '\[External Beta trial\]\(docs/external-beta-trial\.md\)' \
    "README External Beta trial link"
  require_pattern docs/alpha-feedback-triage.md \
    'Do not close route misses only because the read-less ratio looks good' \
    "Alpha feedback triage route miss caveat"
  require_pattern docs/alpha-feedback-triage.md \
    'External Beta Intake' \
    "Alpha feedback triage External Beta intake section"
  require_pattern docs/alpha-feedback-triage.md \
    'replace it with `route_hit`' \
    "Alpha feedback triage needs triage reclassification guidance"
  require_pattern docs/alpha-feedback-triage.md \
    'Large Repository Friction' \
    "Alpha feedback triage large repository friction section"
  require_pattern docs/external-beta-trial.md \
    'scripts/external-beta-trial\.sh /path/to/repo' \
    "External Beta trial command"
  require_pattern docs/external-beta-trial.md \
    'scripts/external-beta-cohort-summary\.sh' \
    "External Beta cohort summary command"
  require_pattern docs/external-beta-trial.md \
    'fails until at least three reports are present' \
    "External Beta cohort check gate"
  require_pattern docs/external-beta-trial.md \
    'redaction-checklist\.md' \
    "External Beta redaction artifact"
  require_pattern docs/external-beta-trial.md \
    'Adoption feedback' \
    "External Beta GitHub issue form guidance"
  require_pattern docs/external-beta-trial.md \
    'paste `issue-body\.md` into' \
    "External Beta issue body paste guidance"
  require_pattern docs/external-beta-trial.md \
    'needs_triage' \
    "External Beta needs triage outcome"
  require_pattern docs/alpha-feedback-triage.md \
    'to execute and return a valid outline for the selected file' \
    "Alpha feedback triage external MCP first-call rule"
  require_pattern docs/alpha-trial-log.md \
    'Next\.js app router' \
    "Alpha trial log Next.js workflow friction row"
  require_pattern docs/alpha-trial-log.md \
    'Maintainer-Run Cohort' \
    "Alpha trial log maintainer-run cohort section"
  require_pattern docs/alpha-trial-log.md \
    '\[#1\]\(https://github\.com/sleticalboy/CodeInsight-mcp/issues/1\)' \
    "Alpha trial log ip2region issue link"
  require_pattern docs/alpha-trial-log.md \
    '\[#2\]\(https://github\.com/sleticalboy/CodeInsight-mcp/issues/2\)' \
    "Alpha trial log mcp-hub issue link"
  require_pattern docs/alpha-trial-log.md \
    '\[#3\]\(https://github\.com/sleticalboy/CodeInsight-mcp/issues/3\)' \
    "Alpha trial log lazy-mcp-wrapper issue link"
  require_pattern .github/ISSUE_TEMPLATE/adoption-feedback.yml \
    'name: Adoption feedback' \
    "GitHub adoption feedback issue form"
  require_pattern .github/ISSUE_TEMPLATE/adoption-feedback.yml \
    'scripts/external-beta-trial\.sh' \
    "GitHub adoption feedback External Beta command"
  require_pattern .github/ISSUE_TEMPLATE/adoption-feedback.yml \
    'issue-body\.md' \
    "GitHub adoption feedback External Beta issue body guidance"
  require_pattern .github/ISSUE_TEMPLATE/adoption-feedback.yml \
    'route_near_miss' \
    "GitHub adoption feedback outcome category"
  require_pattern .github/ISSUE_TEMPLATE/adoption-feedback.yml \
    'needs_triage' \
    "GitHub adoption feedback needs triage category"
  require_pattern scripts/mcp-first-call-smoke.sh \
    'default_fixture = os\.environ\.get\("DEFAULT_FIXTURE"\) == "1"' \
    "MCP first-call default fixture flag"
  require_pattern scripts/mcp-first-call-failure-smoke.sh \
    'external first-call root without a main symbol should pass' \
    "MCP first-call external non-main regression"
  require_pattern docs/README.md \
    '\[Django adoption case\]\(adoption-case-django\.md\)' \
    "docs index Django adoption case link"
  require_pattern docs/README.md \
    '\[Express adoption case\]\(adoption-case-express\.md\)' \
    "docs index Express adoption case link"
  require_pattern docs/README.md \
    '\[Gin adoption case\]\(adoption-case-gin\.md\)' \
    "docs index Gin adoption case link"
  require_pattern docs/README.md \
    '\[ip2region adoption case\]\(adoption-case-ip2region\.md\)' \
    "docs index ip2region adoption case link"
  require_pattern docs/README.md \
    '\[Memchr adoption case\]\(adoption-case-memchr\.md\)' \
    "docs index Memchr adoption case link"
  require_pattern docs/README.md \
    '\[Requests adoption case\]\(adoption-case-requests\.md\)' \
    "docs index Requests adoption case link"
  require_pattern docs/README.md \
    'Local repository benchmark' \
    "docs index local benchmark validation row"
  require_pattern docs/adoption-case-django.md \
    'Commit: `dca76b15c62a1118325b71678ce3235e2231198d`' \
    "Django adoption case commit"
  require_pattern docs/adoption-case-django.md \
    'Read less | `892\.8x`' \
    "Django adoption case read-less metric"
  require_pattern docs/adoption-case-django.md \
    'First selected file \| `django/urls/resolvers\.py`' \
    "Django adoption case first selected file"
  require_pattern docs/adoption-case-django.md \
    'First reading focus \| Start with seed file route registration, matching, or handler dispatch boundaries\.' \
    "Django adoption case first reading focus"
  require_pattern docs/adoption-case-django.md \
    'Generated with: `scripts/update-adoption-case\.sh django`' \
    "Django adoption case generator"
  require_pattern docs/adoption-case-django.md \
    'scripts/update-adoption-case\.sh django --commit dca76b15c62a1118325b71678ce3235e2231198d' \
    "Django adoption case exact refresh command"
  require_pattern docs/adoption-case-django.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-django' \
    "Django adoption case reproduce command"
  require_pattern docs/adoption-case-express.md \
    'Commit: `ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4`' \
    "Express adoption case commit"
  require_pattern docs/adoption-case-express.md \
    'Read less | `92\.6x`' \
    "Express adoption case read-less metric"
  require_pattern docs/adoption-case-express.md \
    'First reading focus \| Start with seed file context and primary symbols\.' \
    "Express adoption case first reading focus"
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
    'Read less | `97\.2x`' \
    "Gin adoption case read-less metric"
  require_pattern docs/adoption-case-gin.md \
    'First selected file \| `routergroup\.go`' \
    "Gin adoption case first selected file"
  require_pattern docs/adoption-case-gin.md \
    'First reading focus \| Start with seed file context and primary symbols\.' \
    "Gin adoption case first reading focus"
  require_pattern docs/adoption-case-gin.md \
    'Generated with: `scripts/update-adoption-case\.sh gin`' \
    "Gin adoption case generator"
  require_pattern docs/adoption-case-gin.md \
    'scripts/update-adoption-case\.sh gin --commit 34dac209ffb6ef85cc78c5d217bbb7ad001d68fd' \
    "Gin adoption case exact refresh command"
  require_pattern docs/adoption-case-gin.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-gin' \
    "Gin adoption case reproduce command"
  require_pattern docs/adoption-case-ip2region.md \
    'Commit: `1a29562c2ddab00e26609f401afa921ed89af263`' \
    "ip2region adoption case commit"
  require_pattern docs/adoption-case-ip2region.md \
    'Read less | `30\.2x`' \
    "ip2region adoption case read-less metric"
  require_pattern docs/adoption-case-ip2region.md \
    'First selected file \| `binding/java/src/main/java/org/lionsoul/ip2region/service/Ip2Region\.java`' \
    "ip2region adoption case first selected file"
  require_pattern docs/adoption-case-ip2region.md \
    'Generated with: `scripts/update-adoption-case\.sh ip2region`' \
    "ip2region adoption case generator"
  require_pattern docs/adoption-case-ip2region.md \
    'scripts/update-adoption-case\.sh ip2region --commit 1a29562c2ddab00e26609f401afa921ed89af263' \
    "ip2region adoption case exact refresh command"
  require_pattern docs/adoption-case-ip2region.md \
    'scripts/adoption-comparison\.sh /tmp/codeinsight-case-ip2region' \
    "ip2region adoption case reproduce command"
  require_pattern docs/adoption-case-memchr.md \
    'Commit: `bce7df7140acff420478a358cde5587904000cb1`' \
    "Memchr adoption case commit"
  require_pattern docs/adoption-case-memchr.md \
    'Read less | `301\.7x`' \
    "Memchr adoption case read-less metric"
  require_pattern docs/adoption-case-memchr.md \
    'First reading focus \| Start with seed file context and primary symbols\.' \
    "Memchr adoption case first reading focus"
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
    'First selected file \| `src/requests/sessions\.py`' \
    "Requests adoption case first selected file"
  require_pattern docs/adoption-case-requests.md \
    'First reading focus \| Start with seed file context and primary symbols\.' \
    "Requests adoption case first reading focus"
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
    'Blind first-read baseline: `675,772` source lines' \
    "adoption cases aggregate baseline"
  require_pattern docs/adoption-cases.md \
    'Aggregate first-read reduction: `99\.6%`' \
    "adoption cases aggregate reduction"
  require_pattern docs/adoption-cases.md \
    'Aggregate read-less ratio: `260\.4x`' \
    "adoption cases aggregate read-less ratio"
  require_pattern docs/adoption-cases.md \
    '\| Case \| Commit \| Seed strategy \| First selected file \| First reading focus \|' \
    "adoption cases route focus column"
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
    '\[case\]\(adoption-case-django\.md\)' \
    "adoption cases Django detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-express\.md\)' \
    "adoption cases Express detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-gin\.md\)' \
    "adoption cases Gin detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-ip2region\.md\)' \
    "adoption cases ip2region detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-memchr\.md\)' \
    "adoption cases Memchr detail link"
  require_pattern docs/adoption-cases.md \
    '\[case\]\(adoption-case-requests\.md\)' \
    "adoption cases Requests detail link"
  require_pattern docs/task-routing-matrix.md \
    '^# Task Routing Matrix$' \
    "task routing matrix doc title"
  require_pattern docs/task-routing-matrix.md \
    'scripts/task-routing-matrix\.sh /path/to/repo' \
    "task routing matrix doc command"
  require_pattern docs/task-routing-matrix.md \
    'The default matrix covers routing, authentication, authorization/access-control,' \
    "task routing matrix default task scope"
  require_pattern docs/task-routing-matrix.md \
    '\-\-expect "understand routing behavior=src/router\.ts"' \
    "task routing matrix expectation example"
  require_pattern docs/task-routing-matrix.md \
    '\-\-expect-file \./route-expectations\.tsv' \
    "task routing matrix expectation file example"
  require_pattern docs/task-routing-matrix.md \
    'Does an authorization task start at permission or token boundary code' \
    "task routing matrix authorization framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a feature flag task start at rollout, toggle, or experiment code' \
    "task routing matrix feature flag framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a network task start at proxy, redirect, adapter, or transport code' \
    "task routing matrix network framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a TLS task start at certificate verification or SSL transport code' \
    "task routing matrix TLS framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a validation task start at schema, binding, parser, or serializer code' \
    "task routing matrix validation framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a persistence task start at database, repository, or storage code' \
    "task routing matrix persistence framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a debugging task start at error handling, retry, or timeout code' \
    "task routing matrix debug framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a coverage task start at test, spec, or regression code' \
    "task routing matrix coverage framing"
  require_pattern docs/task-routing-matrix.md \
    'Does an API handler task start at handler, controller, or endpoint code' \
    "task routing matrix api handler framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a performance task start at cache, latency, or optimization code' \
    "task routing matrix performance framing"
  require_pattern docs/task-routing-matrix.md \
    'Does an observability task start at logs, metrics, telemetry, or tracing code' \
    "task routing matrix observability framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a security task start at sanitization, secrets, or vulnerability code' \
    "task routing matrix security framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a billing task start at payment, checkout, invoice, or subscription code' \
    "task routing matrix billing framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a frontend task start at UI, component, page, or layout code' \
    "task routing matrix frontend framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a background task start at queue, worker, job, or scheduler code' \
    "task routing matrix background framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a documentation task start at docs, guide, or usage example code' \
    "task routing matrix documentation framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a request lifecycle task start at app dispatch, hooks, or response finalization code' \
    "task routing matrix request lifecycle framing"
  require_pattern docs/task-routing-matrix.md \
    'Does a middleware task start at middleware registration or handler boundary code' \
    "task routing matrix middleware framing"
  require_pattern docs/task-routing-matrix.md \
    $'understand authorization permissions\tsrc/permissions\\.ts' \
    "task routing matrix authorization expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand access control rules\tsrc/permissions\\.ts' \
    "task routing matrix access control expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand feature flag rollout\tsrc/feature_flags\\.ts' \
    "task routing matrix feature flag expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand proxy redirect transport\tsrc/network\\.ts' \
    "task routing matrix network expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand ssl certificate verification\tsrc/tls_transport\\.ts' \
    "task routing matrix TLS expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand json binding validation\tsrc/validation\\.ts' \
    "task routing matrix validation expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand persistence behavior\tsrc/database\\.ts' \
    "task routing matrix persistence expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'debug retry timeout handling\tsrc/retry_transport\\.ts' \
    "task routing matrix debug expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'find regression coverage\tsrc/router\\.test\\.ts' \
    "task routing matrix coverage expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand api handler behavior\tsrc/handler\\.ts' \
    "task routing matrix api handler expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand cache performance latency\tsrc/cache\\.ts' \
    "task routing matrix performance expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand observability telemetry logs\tsrc/telemetry\\.ts' \
    "task routing matrix observability expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand security sanitization vulnerabilities\tsrc/security\\.ts' \
    "task routing matrix security expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand checkout subscription payment\tsrc/billing\\.ts' \
    "task routing matrix billing expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand frontend component rendering\tsrc/component\\.tsx' \
    "task routing matrix frontend expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand background job queue\tsrc/worker\\.ts' \
    "task routing matrix background expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand documentation usage\tdocs/usage\\.ts' \
    "task routing matrix documentation expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand request lifecycle before after request handling\tsrc/application\\.ts' \
    "task routing matrix request lifecycle expectation example"
  require_pattern docs/task-routing-matrix.md \
    $'understand middleware behavior\tsrc/middleware\\.ts' \
    "task routing matrix middleware expectation example"
  require_pattern docs/task-routing-matrix.md \
    'Expectation files automatically add their tasks to the matrix' \
    "task routing matrix expectation file task loading"
  require_pattern docs/task-routing-matrix.md \
    '\[Express\]\(task-routing-expectations/express\.tsv\)' \
    "task routing matrix Express expectation example link"
  require_pattern docs/task-routing-matrix.md \
    '\[FastAPI\]\(task-routing-expectations/fastapi\.tsv\)' \
    "task routing matrix FastAPI expectation example link"
  require_pattern docs/task-routing-matrix.md \
    '\[Flask\]\(task-routing-expectations/flask\.tsv\)' \
    "task routing matrix Flask expectation example link"
  require_pattern docs/task-routing-matrix.md \
    '\[Gin\]\(task-routing-expectations/gin\.tsv\)' \
    "task routing matrix Gin expectation example link"
  require_pattern docs/task-routing-matrix.md \
    '\[Requests\]\(task-routing-expectations/requests\.tsv\)' \
    "task routing matrix Requests expectation example link"
  require_pattern docs/task-routing-matrix.md \
    '\[Streamlit\]\(task-routing-expectations/streamlit\.tsv\)' \
    "task routing matrix Streamlit expectation example link"
  require_pattern docs/task-routing-matrix.md \
    'expectations\.checks\[\]\.actual_first_file' \
    "task routing matrix expectation JSON contract"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand middleware behavior\tlib/application\\.js' \
    "Express task routing expectation middleware row"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand express response cookie behavior\tlib/response\\.js' \
    "Express task routing expectation response cookie row"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand express HTTP method routing behavior\tlib/application\\.js' \
    "Express task routing expectation HTTP method row"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand express mounted app router behavior\tlib/application\\.js' \
    "Express task routing expectation mounted router row"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand express request dispatch lifecycle behavior\tlib/application\\.js' \
    "Express task routing expectation request lifecycle row"
  require_pattern docs/task-routing-expectations/express.tsv \
    $'understand express 404 not found final handler behavior\tlib/application\\.js' \
    "Express task routing expectation route miss row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin engine routing behavior\troutergroup\\.go' \
    "Gin task routing expectation routing row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand binding validation behavior\tbinding/default_validator\\.go' \
    "Gin task routing expectation binding validation row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand json binding behavior\tbinding/json\\.go' \
    "Gin task routing expectation JSON binding row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand binding validation test coverage\tbinding/default_validator_test\\.go' \
    "Gin task routing expectation binding validation coverage row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin response cookie behavior\tcontext\\.go' \
    "Gin task routing expectation response cookie row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin route URL path joining behavior\troutergroup\\.go' \
    "Gin task routing expectation URL path joining row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin HTTP method routing behavior\troutergroup\\.go' \
    "Gin task routing expectation HTTP method row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin route group behavior\troutergroup\\.go' \
    "Gin task routing expectation route group row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin request context handler chain behavior\tgin\\.go' \
    "Gin task routing expectation request handler chain row"
  require_pattern docs/task-routing-expectations/gin.tsv \
    $'understand gin no route no method behavior\tgin\\.go' \
    "Gin task routing expectation route miss row"
  require_pattern docs/task-routing-expectations/flask.tsv \
    $'understand flask testing client coverage\ttests/test_testing\\.py' \
    "Flask task routing expectation testing coverage row"
  require_pattern docs/task-routing-expectations/flask.tsv \
    $'understand flask url_for URL building behavior\tsrc/flask/helpers\\.py' \
    "Flask task routing expectation URL building row"
  require_pattern docs/task-routing-expectations/flask.tsv \
    $'understand flask HTTP method dispatch behavior\tsrc/flask/views\\.py' \
    "Flask task routing expectation HTTP method row"
  require_pattern docs/task-routing-expectations/flask.tsv \
    $'understand flask blueprint routing behavior\tsrc/flask/sansio/blueprints\\.py' \
    "Flask task routing expectation blueprint row"
  require_pattern docs/task-routing-expectations/flask.tsv \
    $'understand flask not found method not allowed routing behavior\tsrc/flask/app\\.py' \
    "Flask task routing expectation route miss row"
  require_pattern docs/task-routing-expectations/fastapi.tsv \
    $'understand fastapi dependency injection behavior\tfastapi/dependencies/utils\\.py' \
    "FastAPI task routing expectation dependency injection row"
  require_pattern docs/task-routing-expectations/fastapi.tsv \
    $'understand fastapi websocket behavior\tfastapi/websockets\\.py' \
    "FastAPI task routing expectation websocket row"
  require_pattern docs/task-routing-expectations/fastapi.tsv \
    $'understand fastapi openapi schema generation behavior\tfastapi/openapi/models\\.py' \
    "FastAPI task routing expectation OpenAPI schema row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'understand configuration settings\tsrc/requests/sessions\\.py' \
    "Requests task routing expectation settings row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'understand proxy behavior\tsrc/requests/adapters\\.py' \
    "Requests task routing expectation proxy row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'understand redirect behavior\tsrc/requests/sessions\\.py' \
    "Requests task routing expectation redirect row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'debug retry timeout handling\tsrc/requests/adapters\\.py' \
    "Requests task routing expectation retry timeout row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'understand ssl certificate verification behavior\tsrc/requests/adapters\\.py' \
    "Requests task routing expectation TLS certificate row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'debug certificate verification failure\tsrc/requests/adapters\\.py' \
    "Requests task routing expectation certificate failure row"
  require_pattern docs/task-routing-expectations/requests.tsv \
    $'understand requests adapter test coverage\ttests/test_requests\\.py' \
    "Requests task routing expectation adapter coverage row"
  require_pattern docs/task-routing-expectations/streamlit.tsv \
    $'understand streamlit server startup flow\tlib/streamlit/web/bootstrap\\.py' \
    "Streamlit task routing expectation startup row"
  require_pattern docs/task-routing-expectations/streamlit.tsv \
    $'understand configuration settings\tlib/streamlit/config\\.py' \
    "Streamlit task routing expectation settings row"
  require_pattern docs/task-routing-matrix.md \
    '\| understand gin engine routing behavior \| `routergroup\.go` \|' \
    "task routing matrix Gin routing example"
  require_pattern docs/task-routing-matrix.md \
    '\| understand middleware authentication behavior \| `auth\.go` \|' \
    "task routing matrix Gin auth example"
  require_pattern docs/task-routing-matrix.md \
    '\| understand streamlit server startup flow \| `lib/streamlit/web/bootstrap\.py` \|' \
    "task routing matrix Streamlit startup example"
  require_pattern docs/task-routing-matrix.md \
    '\| understand configuration settings \| `lib/streamlit/config\.py` \|' \
    "task routing matrix Streamlit settings example"
  require_pattern docs/adoption-report-codeinsight.md \
    'CodeInsight routed first-read \| `440` source lines' \
    "CodeInsight self adoption report routed lines"
  require_pattern docs/adoption-report-codeinsight.md \
    'Source lines avoided \| `[0-9]+`' \
    "CodeInsight self adoption report avoided lines"
  require_pattern docs/adoption-report-codeinsight.md \
    'First-read reduction \| `98\.9%`' \
    "CodeInsight self adoption report reduction"
  require_pattern docs/adoption-report-codeinsight.md \
    'Read less \| `[0-9.]+x`' \
    "CodeInsight self adoption report read-less ratio"
  require_pattern docs/adoption-report-codeinsight.md \
    'Type-relation graph filter \| `base_type`' \
    "CodeInsight self adoption report type-relation graph filter"
  require_pattern docs/adoption-report-codeinsight.md \
    'Reading order starts with selected context \| `true`' \
    "CodeInsight self adoption report reading order contract"
  require_pattern docs/adoption-report-codeinsight.md \
    'First execution instruction carries read-less evidence \| `true`' \
    "CodeInsight self adoption report read-less instruction contract"
  require_pattern docs/adoption-report-codeinsight.md \
    'Suggested tool executed through MCP `tools/call` \| `true`' \
    "CodeInsight self adoption report suggested tool contract"
  require_pattern docs/adoption-report-codeinsight.md \
    'First-read gating: suggested_tool_after_selected_context=`true`, continuation_after_selected_context=`true`, impact_review_before_edits=`true`' \
    "CodeInsight self adoption report first-read gating line"
  require_pattern docs/adoption-report-codeinsight.md \
    '/tmp/codeinsight-self-adoption-report\.tar\.gz' \
    "CodeInsight self adoption report archive path"
  require_pattern docs/adoption-report-codeinsight.md \
    'mcp-first-call\.json' \
    "CodeInsight self adoption report MCP artifact"
  require_pattern docs/adoption-report-codeinsight.md \
    'scripts/update-self-adoption-report\.sh --check' \
    "CodeInsight self adoption report updater check command"
  require_pattern docs/adoption-checklist.md \
    'scripts/update-self-adoption-report\.sh --check' \
    "adoption checklist self adoption report check command"
  require_pattern scripts/update-self-adoption-report.sh \
    'Refreshes docs/adoption-report-codeinsight\.md from a live adoption-report run' \
    "CodeInsight self adoption report updater purpose"
  require_pattern scripts/update-self-adoption-report.sh \
    'Verify the checked-in report is already up to date' \
    "CodeInsight self adoption report updater check mode"
  "$ROOT_DIR/scripts/update-adoption-cases.sh" --check >/dev/null
  require_pattern scripts/update-adoption-case.sh \
    'Refreshes a checked-in adoption case from a live adoption-comparison run' \
    "Express adoption case update script purpose"
  require_pattern scripts/update-adoption-case.sh \
    'django\)' \
    "Django adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'gin\)' \
    "Gin adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'ip2region\)' \
    "ip2region adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'memchr\)' \
    "Memchr adoption case update script branch"
  require_pattern scripts/update-adoption-case.sh \
    'requests\)' \
    "Requests adoption case update script branch"
  require_pattern README.md \
    'route `context_pack` first for 4/4 repositories' \
    "context_pack benchmark claim"
  require_readme_benchmark_summary_sync
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
    'first_reading_focus' \
    "benchmark first reading focus guardrail"
  require_pattern scripts/benchmark-smoke.sh \
    'first_reading_question' \
    "benchmark first reading question guardrail"
  require_pattern scripts/benchmark-smoke.sh \
    'first_selection_rank' \
    "benchmark first selection rank guardrail"
  require_pattern scripts/benchmark-smoke.sh \
    '\| File \| Rank \| Focus \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "benchmark reading-plan rank and focus columns"

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
    '\| File \| Rank \| Focus \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "smoke context reading plan rank and focus columns"
  require_pattern docs/benchmark-v0.1.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "smoke reading plan guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_reading_focus` \| present \|' \
    "smoke reading focus guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_reading_question` \| present \|' \
    "smoke reading question guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_reading_reason` \| present \|' \
    "smoke reading reason guardrail"
  require_pattern docs/benchmark-v0.1.md \
    '\| `first_selection_rank` \| >= 1 \|' \
    "smoke selection rank guardrail"
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
    '\| File \| Rank \| Focus \| Question \| Next action \| Suggested tool \| Reason \| Selection reason \|' \
    "large context reading plan rank and focus columns"
  require_pattern docs/benchmark-large.md \
    '\| `reading_plan_steps` \| >= [0-9]+' \
    "large reading plan guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_reading_focus` \| present \|' \
    "large reading focus guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_reading_question` \| present \|' \
    "large reading question guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_reading_reason` \| present \|' \
    "large reading reason guardrail"
  require_pattern docs/benchmark-large.md \
    '\| `first_selection_rank` \| >= 1 \|' \
    "large selection rank guardrail"
  require_context_guardrail_report_sync large docs/benchmark-large.md
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-v0.1.md" smoke
  "$ROOT_DIR/scripts/benchmark-report-smoke.sh" "$ROOT_DIR/docs/benchmark-large.md" large

  require_pattern docs/demo-script.md \
    'scripts/two-minute-demo\.sh' \
    "two-minute demo command"
  require_pattern docs/demo-script.md \
    'scripts/agent-router-demo\.sh' \
    "agent-router raw metrics command"
  require_pattern docs/demo-script.md \
    'scripts/framework-entrypoint-demo\.sh' \
    "framework entrypoint demo command"
  require_pattern README.md \
    'scripts/two-minute-demo\.sh' \
    "README two-minute demo command"
  require_pattern README.md \
    'scripts/framework-entrypoint-demo\.sh' \
    "README framework entrypoint demo command"
  require_pattern README.md \
    'scripts/task-routing-matrix\.sh /path/to/repo' \
    "README task routing matrix command"
  require_pattern README.md \
    '\-\-expect-file \./route-expectations\.tsv' \
    "README task routing matrix expectation file"
  require_pattern README.md \
    '7,101,630 task source lines' \
    "README public route-quality headline"
  require_pattern README.md \
    'Express, FastAPI, Flask, Gin, Requests, and Streamlit expectation files' \
    "README public route matrix default cases"
  require_pattern README.md \
    'first suggested tool such as `file_outline`' \
    "README public route-quality suggested tool evidence"
  require_pattern README.md \
    '\[JSON summary\]\(docs/public-task-routing-matrix-summary\.json\)' \
    "README public route-quality JSON summary link"
  require_pattern README.md \
    'scripts/update-public-task-routing-matrix\.sh --check' \
    "README public route-quality snapshot check command"
  require_pattern README.md \
    'scripts/update-public-task-routing-matrix-smoke\.sh' \
    "README public route-quality no-network smoke command"
  require_pattern README.md \
    '\[public routing snapshot\]\(docs/public-task-routing-matrix\.md\)' \
    "README public routing snapshot link"
  require_pattern README.md \
    'Pick the validation that matches your adoption stage:' \
    "README validation chooser"
  require_pattern README.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/mcp-first-call-smoke\.sh' \
    "README MCP first-call smoke command"
  require_pattern README.md \
    'exposes read-less metrics, selection rank, and continuation evidence' \
    "README MCP first-call candidate evidence proof"
  require_pattern README.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "README installed adoption coverage"
  require_pattern docs/quickstart.md \
    'scripts/two-minute-demo\.sh' \
    "quickstart two-minute demo command"
  require_pattern docs/quickstart.md \
    'scripts/framework-entrypoint-demo\.sh' \
    "quickstart framework entrypoint demo command"
  require_pattern docs/quickstart.md \
    'scripts/task-routing-matrix\.sh /path/to/repo' \
    "quickstart task routing matrix command"
  require_pattern docs/quickstart.md \
    '\-\-expect-file \./route-expectations\.tsv' \
    "quickstart task routing matrix expectation file"
  require_pattern docs/README.md \
    '\[Public task routing matrix\]\(public-task-routing-matrix\.md\)' \
    "docs index public task routing matrix link"
  require_pattern docs/README.md \
    '\[JSON summary\]\(public-task-routing-matrix-summary\.json\)' \
    "docs index public task routing matrix JSON link"
  require_pattern docs/README.md \
    'scripts/update-public-task-routing-matrix\.sh' \
    "docs index public task routing matrix update command"
  require_pattern docs/task-routing-matrix.md \
    '\[public task routing matrix\]\(public-task-routing-matrix\.md\)' \
    "task routing matrix public snapshot link"
  require_pattern docs/task-routing-matrix.md \
    'scripts/update-public-task-routing-matrix\.sh --check' \
    "task routing matrix public snapshot check command"
  require_pattern docs/public-task-routing-matrix.md \
    'Snapshot generated by `scripts/update-public-task-routing-matrix\.sh`' \
    "public task routing matrix generation note"
  require_pattern docs/public-task-routing-matrix.md \
    'expectations: 86/86' \
    "public task routing matrix expectations evidence"
  require_pattern docs/public-task-routing-matrix.md \
    'source_lines: 7101630' \
    "public task routing matrix source-line evidence"
  require_pattern docs/public-task-routing-matrix.md \
    'selected_lines: 40636' \
    "public task routing matrix selected-line evidence"
  require_pattern docs/public-task-routing-matrix.md \
    'line_reduction: 99\.42%' \
    "public task routing matrix line-reduction evidence"
  require_pattern docs/public-task-routing-matrix.md \
    'Aggregate line reduction: `99\.42%`' \
    "public task routing matrix aggregate line-reduction summary"
  require_pattern docs/public-task-routing-matrix.md \
    '\| Task \| First file \| Focus \| Question \| Suggested tool \| Seed strategy \| First seed \| Reduction \| Tokens \| Impact \|' \
    "public task routing matrix first seed column"
  require_pattern docs/task-routing-matrix.md \
    'pinned Express, FastAPI, Flask, Gin, Requests, and Streamlit commits' \
    "task routing matrix default public cases"
  require_pattern docs/public-task-routing-matrix.md \
    '\[`public-task-routing-matrix-summary\.json`\]\(public-task-routing-matrix-summary\.json\)' \
    "public task routing matrix JSON summary link"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"generated_by": "scripts/update-public-task-routing-matrix\.sh"' \
    "public task routing matrix JSON generated-by field"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"line_reduction": 99\.4' \
    "public task routing matrix JSON line reduction"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"first_suggested_tool": "file_outline"' \
    "public task routing matrix JSON first suggested tool evidence"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"first_seed_value":' \
    "public task routing matrix JSON first seed evidence"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"repository": "<case-root>/express"' \
    "public task routing matrix JSON normalized repository path"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"repository": "<case-root>/flask"' \
    "public task routing matrix JSON normalized Flask repository path"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express application routing behavior" and .first_file == "lib/express.js" and .first_suggested_tool == "file_outline" and (.first_reading_focus | contains("route registration")) and (.first_reading_question | contains("routes registered")) and (.first_reading_question | contains("dispatched to handlers")))' \
    "public task routing matrix JSON Express generic routing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express response rendering behavior" and .first_file == "lib/response.js" and (.first_reading_focus | contains("response rendering")) and (.first_reading_question | contains("responses rendered")) and (.first_reading_question | contains("output formats")))' \
    "public task routing matrix JSON Express response rendering first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express error handling behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("error handling")) and (.first_reading_question | contains("errors")) and (.first_reading_question | contains("recovery decisions")))' \
    "public task routing matrix JSON Express error handling first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express static file serving behavior" and .first_file == "lib/express.js" and (.first_reading_focus | contains("static file")) and (.first_reading_question | contains("static files")) and (.first_reading_question | contains("filesystem roots")))' \
    "public task routing matrix JSON Express static file serving first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express request body parsing behavior" and .first_file == "lib/express.js" and (.first_reading_focus | contains("request body parsing")) and (.first_reading_question | contains("request bodies parsed")) and (.first_reading_question | contains("content types selected")))' \
    "public task routing matrix JSON Express request body parsing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express redirect response behavior" and .first_file == "lib/response.js" and (.first_reading_focus | contains("response redirect")) and (.first_reading_question | contains("redirect responses built")) and (.first_reading_question | contains("Location headers")))' \
    "public task routing matrix JSON Express redirect response first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express query parameter parsing behavior" and .first_file == "lib/request.js" and (.first_reading_focus | contains("query string")) and (.first_reading_question | contains("query strings parsed")) and (.first_reading_question | contains("URL parameters")))' \
    "public task routing matrix JSON Express query parameter first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express response header behavior" and .first_file == "lib/response.js" and (.first_reading_focus | contains("response headers")) and (.first_reading_question | contains("response headers set")) and (.first_reading_question | contains("Content-Type values")))' \
    "public task routing matrix JSON Express response header first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express route parameter behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("route parameters")) and (.first_reading_question | contains("route parameters captured")) and (.first_reading_question | contains("passed into handlers")))' \
    "public task routing matrix JSON Express route parameter first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express response cookie behavior" and .first_file == "lib/response.js" and (.first_reading_focus | contains("response cookies")) and (.first_reading_question | contains("response cookies created")) and (.first_reading_question | contains("Set-Cookie headers")))' \
    "public task routing matrix JSON Express response cookie first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express HTTP method routing behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("HTTP method routing")) and (.first_reading_question | contains("HTTP methods registered")) and (.first_reading_question | contains("verbs matched")))' \
    "public task routing matrix JSON Express HTTP method first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express mounted app router behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("mounted routers")) and (.first_reading_question | contains("routers mounted")) and (.first_reading_question | contains("nested routes attached")))' \
    "public task routing matrix JSON Express mounted router first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express request dispatch lifecycle behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("request lifecycle")) and (.first_reading_question | contains("request lifecycle hooks")) and (.first_reading_question | contains("response finalization")))' \
    "public task routing matrix JSON Express request lifecycle first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "express") | .routes[] | select(.task == "understand express 404 not found final handler behavior" and .first_file == "lib/application.js" and (.first_reading_focus | contains("route miss")) and (.first_reading_question | contains("404/405 responses")) and (.first_reading_question | contains("method-not-allowed fallbacks")))' \
    "public task routing matrix JSON Express route miss first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "fastapi") | .routes[] | select(.task == "understand fastapi application routing behavior" and .first_file == "fastapi/routing.py" and (.first_reading_focus | contains("route registration")) and (.first_reading_question | contains("routes registered")))' \
    "public task routing matrix JSON FastAPI routing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "fastapi") | .routes[] | select(.task == "understand fastapi dependency injection behavior" and .first_file == "fastapi/dependencies/utils.py" and (.first_reading_focus | contains("dependency injection")) and (.first_reading_question | contains("dependencies declared")))' \
    "public task routing matrix JSON FastAPI dependency injection first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "fastapi") | .routes[] | select(.task == "understand fastapi websocket behavior" and .first_file == "fastapi/websockets.py" and (.first_reading_focus | contains("WebSocket connection")) and (.first_reading_question | contains("WebSocket connections opened")))' \
    "public task routing matrix JSON FastAPI websocket first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "fastapi") | .routes[] | select(.task == "understand fastapi openapi schema generation behavior" and .first_file == "fastapi/openapi/models.py" and (.first_reading_focus | contains("schema")) and (.first_reading_question | contains("schemas applied")))' \
    "public task routing matrix JSON FastAPI OpenAPI schema first-read focus and question"
  require_pattern docs/public-task-routing-matrix.md \
    'understand request lifecycle before after request handling.*`src/flask/app\.py`.*Start with seed file request lifecycle, dispatch, and response finalization flow\..*Where do request lifecycle hooks, dispatch, and response finalization happen here\?' \
    "public task routing matrix Flask lifecycle focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask session cookie security behavior" and .first_file == "src/flask/sessions.py" and (.first_reading_focus | contains("authentication")) and (.first_reading_focus | contains("session boundaries")) and (.first_reading_question | contains("session boundaries")))' \
    "public task routing matrix JSON Flask session cookie security first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask application routing behavior" and .first_file == "src/flask/sansio/scaffold.py" and .first_suggested_tool == "file_outline" and (.first_reading_focus | contains("route registration")) and (.first_reading_question | contains("routes registered")) and (.first_reading_question | contains("dispatched to handlers")))' \
    "public task routing matrix JSON Flask generic routing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask logging observability behavior" and .first_file == "src/flask/logging.py" and (.first_reading_focus | contains("logging")) and (.first_reading_focus | contains("telemetry")) and (.first_reading_question | contains("logs")) and (.first_reading_question | contains("trace spans")))' \
    "public task routing matrix JSON Flask logging observability first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand request lifecycle before after request handling" and .first_file == "src/flask/app.py" and (.first_reading_focus | contains("request lifecycle")) and (.first_reading_focus | contains("response finalization")) and (.first_reading_question | contains("request lifecycle hooks")) and (.first_reading_question | contains("response finalization")))' \
    "public task routing matrix JSON Flask lifecycle first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask template rendering behavior" and .first_file == "src/flask/templating.py" and (.first_reading_focus | contains("response rendering")) and (.first_reading_question | contains("responses rendered")) and (.first_reading_question | contains("output formats")))' \
    "public task routing matrix JSON Flask template rendering first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask testing client coverage" and .first_file == "tests/test_testing.py" and (.first_reading_focus | contains("regression coverage")) and (.first_reading_question | contains("assertions")) and (.first_reading_question | contains("fixtures")))' \
    "public task routing matrix JSON Flask testing coverage first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask static file serving behavior" and .first_file == "src/flask/blueprints.py" and (.first_reading_focus | contains("static file")) and (.first_reading_question | contains("assets")) and (.first_reading_question | contains("file responses")))' \
    "public task routing matrix JSON Flask static file serving first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask request body parsing behavior" and .first_file == "src/flask/wrappers.py" and (.first_reading_focus | contains("request body parsing")) and (.first_reading_question | contains("payloads bound")) and (.first_reading_question | contains("form data")))' \
    "public task routing matrix JSON Flask request body parsing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask redirect response behavior" and .first_file == "src/flask/helpers.py" and (.first_reading_focus | contains("response redirect")) and (.first_reading_question | contains("status codes selected")) and (.first_reading_question | contains("Location headers")))' \
    "public task routing matrix JSON Flask redirect response first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask query string request args behavior" and .first_file == "src/flask/helpers.py" and (.first_reading_focus | contains("query string")) and (.first_reading_question | contains("request args read")) and (.first_reading_question | contains("URL parameters")))' \
    "public task routing matrix JSON Flask query string first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask response header behavior" and .first_file == "src/flask/wrappers.py" and (.first_reading_focus | contains("response headers")) and (.first_reading_question | contains("status metadata written")) and (.first_reading_question | contains("Content-Type values")))' \
    "public task routing matrix JSON Flask response header first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask url_for URL building behavior" and .first_file == "src/flask/helpers.py" and (.first_reading_focus | contains("URL building")) and (.first_reading_focus | contains("route path joining")) and (.first_reading_question | contains("URLs built")) and (.first_reading_question | contains("routes reversed")))' \
    "public task routing matrix JSON Flask URL building first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask HTTP method dispatch behavior" and .first_file == "src/flask/views.py" and (.first_reading_focus | contains("HTTP method routing")) and (.first_reading_question | contains("HTTP methods registered")) and (.first_reading_question | contains("handlers dispatched")))' \
    "public task routing matrix JSON Flask HTTP method first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask blueprint routing behavior" and .first_file == "src/flask/sansio/blueprints.py" and (.first_reading_focus | contains("blueprints")) and (.first_reading_question | contains("blueprints registered")) and (.first_reading_question | contains("nested routes attached")))' \
    "public task routing matrix JSON Flask blueprint first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "flask") | .routes[] | select(.task == "understand flask not found method not allowed routing behavior" and .first_file == "src/flask/app.py" and (.first_reading_focus | contains("route miss")) and (.first_reading_question | contains("not-found handlers")) and (.first_reading_question | contains("method-not-allowed fallbacks")))' \
    "public task routing matrix JSON Flask route miss first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand binding validation behavior" and .first_file == "binding/default_validator.go" and (.first_reading_focus | contains("validation")) and (.first_reading_question | contains("inputs validated")))' \
    "public task routing matrix JSON Gin validation first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin engine routing behavior" and .first_file == "routergroup.go" and .first_suggested_tool == "file_outline" and (.first_reading_focus | contains("route registration")) and (.first_reading_question | contains("routes registered")) and (.first_reading_question | contains("dispatched to handlers")))' \
    "public task routing matrix JSON Gin generic routing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand json binding behavior" and .first_file == "binding/json.go" and (.first_reading_focus | contains("binding")) and (.first_reading_question | contains("payloads bound")))' \
    "public task routing matrix JSON Gin JSON binding first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand binding validation test coverage" and .first_file == "binding/default_validator_test.go" and (.first_reading_focus | contains("regression coverage")) and (.first_reading_question | contains("assertions")) and (.first_reading_question | contains("fixtures")))' \
    "public task routing matrix JSON Gin binding validation coverage first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin response rendering behavior" and .first_file == "render/render.go" and (.first_reading_focus | contains("response rendering")) and (.first_reading_question | contains("responses rendered")) and (.first_reading_question | contains("output formats")))' \
    "public task routing matrix JSON Gin response rendering first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin error recovery middleware behavior" and .first_file == "recovery.go" and (.first_reading_focus | contains("error handling")) and (.first_reading_question | contains("errors")) and (.first_reading_question | contains("recovery decisions")))' \
    "public task routing matrix JSON Gin error recovery first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin static file serving behavior" and .first_file == "routergroup.go" and (.first_reading_focus | contains("static file")) and (.first_reading_question | contains("filesystem roots")) and (.first_reading_question | contains("file responses")))' \
    "public task routing matrix JSON Gin static file serving first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin request body parsing behavior" and .first_file == "context.go" and (.first_reading_focus | contains("request body parsing")) and (.first_reading_question | contains("content types selected")) and (.first_reading_question | contains("form data")))' \
    "public task routing matrix JSON Gin request body parsing first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin redirect response behavior" and .first_file == "context.go" and (.first_reading_focus | contains("response redirect")) and (.first_reading_question | contains("status codes selected")) and (.first_reading_question | contains("Location headers")))' \
    "public task routing matrix JSON Gin redirect response first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin query parameter parsing behavior" and .first_file == "binding/query.go" and (.first_reading_focus | contains("query string")) and (.first_reading_question | contains("query strings parsed")) and (.first_reading_question | contains("URL parameters")))' \
    "public task routing matrix JSON Gin query parameter first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin response header behavior" and .first_file == "context.go" and (.first_reading_focus | contains("response headers")) and (.first_reading_question | contains("response headers set")) and (.first_reading_question | contains("Content-Type values")))' \
    "public task routing matrix JSON Gin response header first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin path parameter behavior" and .first_file == "tree.go" and (.first_reading_focus | contains("path variables")) and (.first_reading_question | contains("attached to requests")) and (.first_reading_question | contains("passed into handlers")))' \
    "public task routing matrix JSON Gin path parameter first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin response cookie behavior" and .first_file == "context.go" and (.first_reading_focus | contains("response cookies")) and (.first_reading_question | contains("response cookies created")) and (.first_reading_question | contains("Set-Cookie headers")))' \
    "public task routing matrix JSON Gin response cookie first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin route URL path joining behavior" and .first_file == "routergroup.go" and (.first_reading_focus | contains("URL building")) and (.first_reading_focus | contains("route path joining")) and (.first_reading_question | contains("URLs built")) and (.first_reading_question | contains("route paths joined")))' \
    "public task routing matrix JSON Gin URL path joining first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin HTTP method routing behavior" and .first_file == "routergroup.go" and (.first_reading_focus | contains("HTTP method routing")) and (.first_reading_question | contains("HTTP methods registered")) and (.first_reading_question | contains("verbs matched")))' \
    "public task routing matrix JSON Gin HTTP method first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin route group behavior" and .first_file == "routergroup.go" and (.first_reading_focus | contains("route groups")) and (.first_reading_question | contains("route groups created")) and (.first_reading_question | contains("nested routes attached")))' \
    "public task routing matrix JSON Gin route group first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin request context handler chain behavior" and .first_file == "gin.go" and (.first_reading_focus | contains("request lifecycle")) and (.first_reading_question | contains("request lifecycle hooks")) and (.first_reading_question | contains("response finalization")))' \
    "public task routing matrix JSON Gin request handler chain first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "gin") | .routes[] | select(.task == "understand gin no route no method behavior" and .first_file == "gin.go" and (.first_reading_focus | contains("route miss")) and (.first_reading_question | contains("404/405 responses")) and (.first_reading_question | contains("not-found handlers")))' \
    "public task routing matrix JSON Gin route miss first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand requests session request flow" and .first_file == "src/requests/sessions.py" and (.first_reading_focus | contains("network client")) and (.first_reading_question | contains("network requests")) and (.first_reading_question | contains("adapters")))' \
    "public task routing matrix JSON Requests session request flow first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand proxy behavior" and .first_file == "src/requests/adapters.py" and (.first_reading_focus | contains("network client")) and (.first_reading_question | contains("proxies")))' \
    "public task routing matrix JSON Requests proxy first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand requests cookie handling behavior" and .first_file == "src/requests/cookies.py" and (.first_reading_focus | contains("cookies")) and (.first_reading_focus | contains("HTTP state")) and (.first_reading_question | contains("cookies")) and (.first_reading_question | contains("HTTP state")))' \
    "public task routing matrix JSON Requests cookie first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand requests headers case insensitive behavior" and .first_file == "src/requests/structures.py" and (.first_reading_focus | contains("headers")) and (.first_reading_focus | contains("HTTP state")) and (.first_reading_question | contains("headers")) and (.first_reading_question | contains("HTTP state")))' \
    "public task routing matrix JSON Requests headers first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand redirect behavior" and .first_file == "src/requests/sessions.py" and (.first_reading_focus | contains("redirect")) and (.first_reading_question | contains("redirects")))' \
    "public task routing matrix JSON Requests redirect first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "debug retry timeout handling" and .first_file == "src/requests/adapters.py" and (.first_reading_focus | contains("error handling")) and (.first_reading_question | contains("timeouts")))' \
    "public task routing matrix JSON Requests retry timeout first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand ssl certificate verification behavior" and .first_file == "src/requests/adapters.py" and (.first_reading_focus | contains("TLS")) and (.first_reading_question | contains("TLS certificates")) and (.first_reading_question | contains("verification decisions")))' \
    "public task routing matrix JSON Requests TLS certificate first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "debug certificate verification failure" and .first_file == "src/requests/adapters.py" and (.first_reading_focus | contains("TLS")) and (.first_reading_question | contains("TLS certificates")) and (.first_reading_question | contains("verification decisions")))' \
    "public task routing matrix JSON Requests certificate failure first-read focus and question"
  require_jq docs/public-task-routing-matrix-summary.json \
    '.cases[] | select(.case == "requests") | .routes[] | select(.task == "understand requests adapter test coverage" and .first_file == "tests/test_requests.py" and (.first_reading_focus | contains("regression coverage")) and (.first_reading_question | contains("assertions")) and (.first_reading_question | contains("fixtures")))' \
    "public task routing matrix JSON Requests adapter coverage first-read focus and question"
  require_pattern docs/public-task-routing-matrix-summary.json \
    '"summary_json": "<output-dir>/requests/summary\.json"' \
    "public task routing matrix JSON normalized summary path"
  require_pattern .github/workflows/ci.yml \
    'public-task-routing-matrix-step-summary\.sh docs/public-task-routing-matrix-summary\.json codeinsight-public-routing-snapshot' \
    "public task routing matrix step summary CI command"
  require_pattern scripts/public-task-routing-matrix-step-summary.sh \
    'Public Task Routing Matrix' \
    "public task routing matrix step summary heading"
  require_pattern scripts/public-task-routing-matrix-step-summary.sh \
    'Workflow artifact: \[`%s`\]\(%s\)' \
    "public task routing matrix step summary artifact link"
  require_pattern scripts/public-task-routing-matrix-step-summary-smoke.sh \
    'Workflow artifact: \[`codeinsight-public-routing-snapshot`\]\(https://example\.com/artifact\)' \
    "public task routing matrix step summary smoke artifact link"
  require_pattern docs/public-task-routing-matrix.md \
    'express: 17 tasks, first files index\.js, lib/application\.js, lib/express\.js, lib/request\.js, lib/response\.js' \
    "public task routing matrix express summary"
  require_pattern docs/public-task-routing-matrix.md \
    'fastapi: 12 tasks, first files fastapi/_compat/v2\.py, fastapi/applications\.py, fastapi/background\.py, fastapi/dependencies/utils\.py, fastapi/exceptions\.py, fastapi/openapi/models\.py, fastapi/requests\.py, fastapi/routing\.py, fastapi/security/oauth2\.py, fastapi/websockets\.py, tests/test_fastapi_cli\.py' \
    "public task routing matrix fastapi summary"
  require_pattern docs/public-task-routing-matrix.md \
    'gin: 20 tasks, first files auth\.go, binding/default_validator\.go, binding/default_validator_test\.go, binding/json\.go, binding/query\.go, context\.go, gin\.go, recovery\.go, render/render\.go, routergroup\.go, tree\.go' \
    "public task routing matrix gin summary"
  require_pattern docs/public-task-routing-matrix.md \
    'flask: 17 tasks, first files src/flask/app\.py, src/flask/blueprints\.py, src/flask/cli\.py, src/flask/config\.py, src/flask/helpers\.py, src/flask/logging\.py, src/flask/sansio/blueprints\.py, src/flask/sansio/scaffold\.py, src/flask/sessions\.py, src/flask/templating\.py, src/flask/views\.py, src/flask/wrappers\.py, tests/test_testing\.py' \
    "public task routing matrix flask summary"
  require_pattern docs/public-task-routing-matrix.md \
    'requests: 12 tasks, first files src/requests/__init__\.py, src/requests/adapters\.py, src/requests/auth\.py, src/requests/cookies\.py, src/requests/sessions\.py, src/requests/structures\.py, tests/test_requests\.py' \
    "public task routing matrix requests summary"
  require_pattern docs/public-task-routing-matrix.md \
    'streamlit: 8 tasks, first files lib/streamlit/config\.py, lib/streamlit/runtime/caching/cache_data_api\.py, lib/streamlit/runtime/memory_uploaded_file_manager\.py, lib/streamlit/runtime/scriptrunner/script_runner\.py, lib/streamlit/runtime/secrets\.py, lib/streamlit/runtime/websocket_session_manager\.py, lib/streamlit/web/bootstrap\.py, lib/streamlit/web/server/starlette/starlette_static_routes\.py' \
    "public task routing matrix streamlit summary"
  require_pattern docs/public-task-routing-matrix.md \
    'Where does the runtime execute scripts, coordinate reruns, or transition lifecycle state here\?' \
    "public task routing matrix Streamlit script runner prompt"
  require_pattern docs/public-task-routing-matrix.md \
    'Where are uploaded files stored, retrieved, cleaned up, or exposed to callers here\?' \
    "public task routing matrix Streamlit uploaded file prompt"
  require_pattern docs/public-task-routing-matrix.md \
    'Where are WebSocket connections opened, tracked, handed to sessions, or closed here\?' \
    "public task routing matrix Streamlit websocket prompt"
  require_pattern scripts/update-public-task-routing-matrix.sh \
    '\-\-check' \
    "public task routing matrix update check option"
  require_pattern scripts/update-public-task-routing-matrix.sh \
    'scripts/public-task-routing-matrix\.sh' \
    "public task routing matrix update generator"
  require_pattern scripts/update-public-task-routing-matrix-smoke.sh \
    'update public task routing matrix smoke passed' \
    "public task routing matrix update smoke success output"
  require_pattern docs/maintenance-commands.md \
    'scripts/update-public-task-routing-matrix\.sh --check' \
    "maintenance public task routing matrix snapshot check command"
  require_pattern docs/maintenance-commands.md \
    'scripts/update-public-task-routing-matrix-smoke\.sh' \
    "maintenance public task routing matrix update smoke command"
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
    'source_lines_avoided: 76092' \
    "demo script source lines avoided metric"
  require_pattern docs/demo-script.md \
    'read_less_ratio: 141\.7x' \
    "demo script read-less metric"
  require_pattern docs/demo-script.md \
    'Read less: avoided 76092 source lines, 141\.7x less text before follow-up tools\.' \
    "demo script evidence summary read-less line"
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
  require_pattern docs/first-read-workflow.md \
    'Framework-oriented file signals are also included' \
    "first-read workflow framework entrypoint signals"
  require_pattern docs/first-read-workflow.md \
    'Rails `config/routes\.rb`, and Java' \
    "first-read workflow Rails and Java entrypoints"
  require_pattern docs/first-read-workflow.md \
    'Python web framework signals include `manage\.py`' \
    "first-read workflow Python framework entrypoints"
  require_pattern docs/first-read-workflow.md \
    'C# web application signals include' \
    "first-read workflow C# framework entrypoints"
  require_pattern docs/first-read-workflow.md \
    'Common agent task aliases are expanded before auto-selection' \
    "first-read workflow task alias routing"
  require_pattern docs/first-read-workflow.md \
    '`routing` can match `route`, `routes`, or `router`' \
    "first-read workflow routing aliases"
  require_pattern docs/first-read-workflow.md \
    'Auto-selected seed files keep their seed order' \
    "first-read workflow seed-order routing"
  require_pattern docs/status.md \
    'local-first first-read router for AI coding agents' \
    "status first-read router positioning"
  require_pattern docs/status.md \
    'passes `86/86` expected first-file checks' \
    "status public route-quality pass count"
  require_pattern docs/status.md \
    '40,636 of 7,101,630 task source lines' \
    "status public route-quality read-less evidence"
  require_pattern docs/status.md \
    'first suggested tool for every route' \
    "status public route-quality suggested tool evidence"
  require_pattern docs/status.md \
    'agent_route -> selected context -> executable suggested_tool -> impact check' \
    "status first-read route chain"
  require_pattern docs/status.md \
    'Framework-oriented entrypoint signals' \
    "status framework entrypoint signals"
  require_pattern docs/status.md \
    'Python web framework roots' \
    "status Python framework entrypoint signals"
  require_pattern docs/status.md \
    'C# web' \
    "status C# framework entrypoint signals"
  require_pattern docs/status.md \
    'mirrors `impact_analysis\.suggested_checks`' \
    "status impact execution suggested checks"
  require_pattern docs/status.md \
    '`pnpm test -- src/core\.test\.ts`' \
    "status focused test command example"
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
    'context, first reading focus/question, selection rank, continuation next action' \
    "maintenance agent-route first-read evidence summary"
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
    'first context file, task-path seed evidence, read-less metrics, selection rank, reading-question handoff, continuation summary, reading-plan order, suggested-tool handoff, impact status, blocked no-seed/no-context/unindexed-path handling, and saved artifacts' \
    "maintenance MCP first-call artifact scope"
  require_pattern docs/maintenance-commands.md \
    'scripts/mcp-first-call-step-summary-smoke\.sh' \
    "maintenance MCP first-call step summary smoke"
  require_pattern docs/maintenance-commands.md \
    '\| First MCP call Actions summary changed \| `scripts/mcp-first-call-step-summary-smoke\.sh` \|' \
    "maintenance MCP first-call step summary chooser"
  require_pattern docs/maintenance-commands.md \
    'Actions Summary section for selected files, task-path seed evidence, first context file, first reading file, read-less metrics, selection rank, reading-question handoff, omitted-candidate continuation fields, reading-plan order, suggested-tool handoff, continuation timing, impact status, blocked no-seed/no-context/unindexed-path handling, and artifact link' \
    "maintenance MCP first-call step summary scope"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Seed strategy: `auto_task_path`' \
    "MCP first-call step summary seed strategy"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First seed source: `task_path`' \
    "MCP first-call step summary first seed source"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First context file: `src/auth\.ts`' \
    "MCP first-call step summary first context file"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First reading selection rank: `1`' \
    "MCP first-call step summary first reading selection rank"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Read less: `10\.0x`' \
    "MCP first-call step summary read-less metric"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Blocked no-seed next action: `provide_seed_file_or_symbol`' \
    "MCP first-call step summary blocked no-seed next action"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Reading order contract: `true`' \
    "MCP first-call step summary reading order contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First execution instruction focus contract: `true`' \
    "MCP first-call step summary execution instruction focus contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First execution instruction question contract: `true`' \
    "MCP first-call step summary execution instruction question contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Suggested tool handoff contract: `true`' \
    "MCP first-call step summary suggested tool handoff contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Current-step instruction focus contract: `true`' \
    "MCP first-call step summary current-step instruction focus contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'Continuation timing contract: `true`' \
    "MCP first-call step summary continuation timing contract"
  require_pattern scripts/mcp-first-call-step-summary-smoke.sh \
    'First omitted omission reason: `-`' \
    "MCP first-call step summary omitted reason"
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
    'executable suggested-tool calls, read-less metrics, selection rank, and continuation evidence' \
    "maintenance MCP smoke selection evidence scope"
  require_pattern docs/maintenance-commands.md \
    '\| Framework entrypoint routing changed \| `scripts/framework-entrypoint-demo\.sh` \|' \
    "maintenance framework entrypoint smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'Temporary multi-framework fixture covering Next\.js, Rails, Django, and C# web first-context selection' \
    "maintenance framework entrypoint smoke scope"
  require_pattern docs/maintenance-commands.md \
    '\| Task alias or seed ordering changed \| `scripts/task-routing-matrix-smoke\.sh` \|' \
    "maintenance task routing matrix smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'routing, authentication, authorization, access-control, settings, feature flag, network, TLS, validation, startup, persistence, debug, coverage, API handler, cache, observability, security, billing, frontend, background job, documentation, request lifecycle, middleware, and AI-agent first-read prompts choose the matching first file and that `--expect-file` failures are reported' \
    "maintenance task routing matrix smoke scope"
  require_pattern docs/maintenance-commands.md \
    '\| Installed-binary adoption path changed \| `CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh` \|' \
    "maintenance installed binary smoke chooser"
  require_pattern docs/maintenance-commands.md \
    'installed binary, including read-less metrics, selection rank, and continuation evidence' \
    "maintenance installed binary selection evidence scope"
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
  require_pattern docs/mcp-client-config.md \
    '`selection_rank`, `omission_reason`, and `next_action` fields let clients show' \
    "MCP client omitted candidate explanation fields"
  require_pattern docs/recommendation-contract.md \
    '`selection_rank`, `omission_reason`, and `next_action` for machine-readable' \
    "recommendation contract omitted candidate explanation fields"
  require_pattern docs/recommendation-contract.md \
    '`source_lines_avoided`: non-negative baseline minus selected lines' \
    "recommendation contract read-less avoided metric"
  require_pattern docs/recommendation-contract.md \
    '`suggested_checks\[\]`: optional command or review checks' \
    "recommendation contract execution suggested checks field"
  require_pattern docs/recommendation-contract.md \
    'source-line reduction evidence; the continuation step names' \
    "recommendation contract execution plan read-less evidence"
  require_pattern docs/recommendation-contract.md \
    'They must still read selected context before using `suggested_tool`' \
    "recommendation contract read-less selected-context boundary"
  require_pattern docs/first-read-workflow.md \
    '`omitted_candidates\[\]\.selection_rank`, `omission_reason`, and' \
    "first-read omitted candidate explanation fields"
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
    'first reading focus/question metrics for selected context' \
    "maintenance context-pack quality first reading focus/question scope"
  require_pattern docs/maintainer-checklist.md \
    'context-pack quality smoke' \
    "maintainer context-pack quality smoke"
  require_pattern docs/maintainer-checklist.md \
    'first reading focus/question metrics' \
    "maintainer context-pack quality first reading focus/question summary"
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
    'compact `routing_decision` line, context-pack metrics, first reading' \
    "maintainer agent-route first-read evidence summary"
  require_pattern docs/maintenance-commands.md \
    'first reading focus/question, selection rank, continuation next action' \
    "maintenance agent-route summary evidence scope"
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
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'first_reading_selection_rank' \
    "MCP first-call artifact first reading selection rank output"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'read_less_ratio' \
    "MCP first-call artifact read-less output"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'first_omitted_omission_reason' \
    "MCP first-call artifact omitted reason output"
  require_pattern scripts/mcp-first-call-artifact-smoke.sh \
    'blocked_unindexed_task_path_status' \
    "MCP first-call artifact unindexed task path output"
  require_pattern docs/maintainer-checklist.md \
    'task-path seed evidence, the first context file, first reading file, read-less' \
    "maintainer MCP first-call route contract summary"
  require_pattern docs/release-readiness.md \
    'task-path seed evidence,' \
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
    'type-relation routing fields' \
    "release commands adoption report type-relation fields"
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
    'scripts/release-handoff-summary\.sh --generate-evidence --json-output release-handoff/vX\.Y\.Z\.json --output release-handoff/vX\.Y\.Z\.md vX\.Y\.Z' \
    "release commands handoff generated evidence summary"
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
  require_pattern docs/release-commands.md \
    'scripts/post-release-verify\.sh --handoff --generate-evidence-for-handoff vX\.Y\.Z' \
    "release commands post-release generated evidence handoff"
  require_pattern docs/release-commands.md \
    'Use `--generate-evidence-for-handoff` when handoff should create a missing JSON' \
    "release commands post-release generated evidence explanation"
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
    'scripts/post-release-verify\.sh --handoff --generate-evidence-for-handoff vX\.Y\.Z' \
    "release runbook post-release generated evidence handoff"
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
  require_pattern scripts/release-handoff-summary.sh \
    'Generate the evidence JSON first' \
    "release handoff generated evidence option"
  require_pattern scripts/release-handoff-summary.sh \
    'RELEASE_EVIDENCE_SUMMARY_SCRIPT' \
    "release handoff evidence summary script hook"
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
  require_pattern scripts/release-handoff-summary.sh \
    'Adoption report type-relation routing' \
    "release handoff adoption report type-relation routing"
  require_pattern scripts/release-notes-draft.sh \
    'Adoption Report Evidence' \
    "release notes adoption report evidence section"
  require_pattern scripts/release-notes-draft.sh \
    'Type-relation routing' \
    "release notes adoption report type-relation routing"
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
    'routing_decision_first_seed' \
    "agent-router routing decision first seed output"
  require_pattern scripts/agent-router-demo.sh \
    'routing_decision_read_less' \
    "agent-router routing decision read-less output"
  require_pattern scripts/agent-router-demo.sh \
    'routing_decision_impact_status' \
    "agent-router routing decision impact output"
  require_pattern scripts/agent-router-demo.sh \
    'first_reading_question' \
    "agent-router first reading question output"
  require_pattern scripts/agent-router-demo.sh \
    'reading_plan_reason' \
    "agent-router reading-plan reason output"
  require_pattern scripts/agent-router-demo.sh \
    'selection_rank' \
    "agent-router selection rank output"
  require_pattern scripts/agent-router-demo.sh \
    'selection_reason' \
    "agent-router selection reason output"
  require_pattern scripts/agent-router-demo.sh \
    'continuation_next_action' \
    "agent-router continuation next action output"
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
    'require_json_string "\$route_json" '\''\.routing_decision\.first_file'\''' \
    "agent-router routing decision assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.question'\''' \
    "agent-router first reading question assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.reading_plan\[0\]\.reason'\''' \
    "agent-router reading-plan reason assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_number_gt_zero "\$context_json" '\''\.reading_plan\[0\]\.selection_rank'\''' \
    "agent-router selection rank assertion"
  require_pattern scripts/agent-router-demo.sh \
    'require_json_string "\$context_json" '\''\.continuation_summary\.next_action'\''' \
    "agent-router continuation next action assertion"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'framework entrypoint demo passed' \
    "framework entrypoint demo success output"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'Next\.js app router entrypoint' \
    "framework entrypoint demo Next.js assertion"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'Rails route entrypoint' \
    "framework entrypoint demo Rails assertion"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'Python web framework entrypoint' \
    "framework entrypoint demo Python assertion"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'C# web application entrypoint' \
    "framework entrypoint demo C# assertion"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'routes_first_context' \
    "framework entrypoint demo routes-first output"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'urls_first_context' \
    "framework entrypoint demo urls-first output"
  require_pattern scripts/framework-entrypoint-demo.sh \
    'csharp_first_context' \
    "framework entrypoint demo csharp-first output"
  require_pattern scripts/task-routing-matrix.sh \
    'CodeInsight Task Routing Matrix' \
    "task routing matrix markdown title"
  require_pattern scripts/task-routing-matrix.sh \
    'understand routing behavior' \
    "task routing matrix default routing task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand authentication behavior' \
    "task routing matrix default authentication task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand authorization permissions' \
    "task routing matrix default authorization task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand access control rules' \
    "task routing matrix default access control task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand feature flag rollout' \
    "task routing matrix default feature flag task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand proxy redirect transport' \
    "task routing matrix default network task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand ssl certificate verification' \
    "task routing matrix default TLS task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand json binding validation' \
    "task routing matrix default validation task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand persistence behavior' \
    "task routing matrix default persistence task"
  require_pattern scripts/task-routing-matrix.sh \
    'debug retry timeout handling' \
    "task routing matrix default debug task"
  require_pattern scripts/task-routing-matrix.sh \
    'find regression coverage' \
    "task routing matrix default coverage task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand api handler behavior' \
    "task routing matrix default api handler task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand cache performance latency' \
    "task routing matrix default performance task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand observability telemetry logs' \
    "task routing matrix default observability task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand security sanitization vulnerabilities' \
    "task routing matrix default security task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand checkout subscription payment' \
    "task routing matrix default billing task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand frontend component rendering' \
    "task routing matrix default frontend task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand background job queue' \
    "task routing matrix default background task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand documentation usage' \
    "task routing matrix default documentation task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand request lifecycle before after request handling' \
    "task routing matrix default request lifecycle task"
  require_pattern scripts/task-routing-matrix.sh \
    'understand middleware behavior' \
    "task routing matrix default middleware task"
  require_pattern scripts/task-routing-matrix.sh \
    'first_selection_reason' \
    "task routing matrix selection reason field"
  require_pattern scripts/task-routing-matrix.sh \
    '\-\-expect TASK=FILE' \
    "task routing matrix expect option"
  require_pattern scripts/task-routing-matrix.sh \
    '\-\-expect-file PATH' \
    "task routing matrix expect-file option"
  require_pattern scripts/task-routing-matrix.sh \
    'expectations' \
    "task routing matrix expectations summary"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'bad-expectations\.json' \
    "task routing matrix smoke JSON expectation file"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'task routing matrix smoke passed' \
    "task routing matrix smoke success output"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'matrix should fail when an expected first file does not match' \
    "task routing matrix smoke expectation failure"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/router\.ts' \
    "task routing matrix smoke router assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/config\.ts' \
    "task routing matrix smoke config assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/feature_flags\.ts' \
    "task routing matrix smoke feature flag assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/network\.ts' \
    "task routing matrix smoke network assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/tls_transport\.ts' \
    "task routing matrix smoke TLS transport assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/validation\.ts' \
    "task routing matrix smoke validation assertion"
  require_pattern scripts/task-routing-matrix-smoke.sh \
    'src/retry_transport\.ts' \
    "task routing matrix smoke retry transport assertion"
  require_pattern tests/cli.rs \
    'context\["reading_plan"\]\[0\]\["selection_rank"\]' \
    "CLI context-pack reading-plan selection rank assertion"
  require_pattern tests/cli.rs \
    'first_omitted\["omission_reason"\]' \
    "CLI context-pack omitted candidate omission reason assertion"
  require_pattern scripts/installed-quickstart-smoke.sh \
    'context_selection_rank' \
    "installed quickstart context selection rank output"
  require_pattern scripts/installed-quickstart-smoke.sh \
    'agent_route_continuation_status' \
    "installed quickstart agent-route continuation output"
  require_pattern scripts/installed-quickstart-smoke.sh \
    'mcp_agent_route_selection_rank' \
    "installed quickstart MCP agent-route selection rank output"
  require_pattern scripts/installed-quickstart-smoke.sh \
    'mcp_agent_route_read_less_ratio' \
    "installed quickstart MCP agent-route read-less output"
  require_pattern scripts/installed-quickstart-smoke.sh \
    'mcp_agent_route_first_omitted_omission_reason' \
    "installed quickstart MCP agent-route omitted reason output"
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
    'agent_route_first_reading_selection_rank' \
    "MCP stdio agent-route selection rank output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route_read_less_ratio' \
    "MCP stdio agent-route read-less output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'explicit_read_less_ratio' \
    "MCP stdio explicit context read-less output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'auto_read_less_ratio' \
    "MCP stdio auto context read-less output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'agent_route_continuation_status' \
    "MCP stdio agent-route continuation status output"
  require_pattern scripts/mcp-stdio-smoke.sh \
    'explicit_first_omitted_omission_reason' \
    "MCP stdio explicit omitted reason output"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First reading question' \
    "agent-route step summary first reading question"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First execution instruction has focus' \
    "agent-route step summary execution instruction focus"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First execution instruction has question' \
    "agent-route step summary execution instruction question"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First execution instruction has read less' \
    "agent-route step summary execution instruction read-less"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'Current-step instruction has focus' \
    "agent-route step summary current-step instruction focus"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'First selection rank' \
    "agent-route step summary first selection rank"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'Continuation next action' \
    "agent-route step summary continuation next action"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'Impact execution suggested tool' \
    "agent-route step summary impact suggested tool"
  require_pattern scripts/agent-route-step-summary-smoke.sh \
    'Impact execution suggested checks' \
    "agent-route step summary impact suggested checks"
  require_pattern scripts/agent-route-smoke.sh \
    'impact_execution_suggested_tool' \
    "agent-route smoke impact execution suggested tool metric"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'first_reading_question' \
    "agent-route artifact first reading question output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'current_reading_step_matches_reading_plan' \
    "agent-route artifact current reading step mirror output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'first_execution_instruction_has_focus' \
    "agent-route artifact first execution instruction focus output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'first_execution_instruction_has_read_less' \
    "agent-route artifact first execution instruction read-less output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'current_step_instruction_has_focus' \
    "agent-route artifact current-step instruction focus output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'first_selection_rank' \
    "agent-route artifact first selection rank output"
  require_pattern scripts/agent-route-artifact-smoke.sh \
    'continuation_next_action' \
    "agent-route artifact continuation next action output"
  require_pattern scripts/release-evidence-summary.sh \
    'agent_route_first_selection_rank' \
    "release evidence agent-route first selection rank output"
  require_pattern scripts/release-evidence-summary.sh \
    'agent_route_continuation_next_action' \
    "release evidence agent-route continuation next action output"
  require_pattern scripts/release-handoff-summary.sh \
    'Agent-route first selection' \
    "release handoff agent-route first selection output"
  require_pattern scripts/release-notes-draft.sh \
    'Agent-route continuation' \
    "release notes agent-route continuation output"
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
    'agent_route_current_reading_step_matches_reading_plan: true' \
    "MCP client smoke current reading step mirror output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_read_less_ratio: 7\.0x' \
    "MCP client smoke read-less output"
  require_pattern docs/mcp-client-smoke.md \
    'explicit_read_less_ratio: 10\.1x' \
    "MCP client smoke explicit read-less output"
  require_pattern docs/mcp-client-smoke.md \
    'auto_read_less_ratio: 7\.0x' \
    "MCP client smoke auto read-less output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_suggested_tool_executed: true' \
    "MCP client smoke execution-plan suggested tool output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_impact_suggested_tool: impact_analysis' \
    "MCP client smoke impact suggested tool output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_impact_suggested_checks' \
    "MCP client smoke impact suggested checks output"
  require_pattern docs/mcp-client-smoke.md \
    'agent_route_first_reading_selection_rank: 1' \
    "MCP client smoke selection rank output"
  require_pattern docs/mcp-client-smoke.md \
    'explicit_first_omitted_omission_reason: token_budget_exhausted' \
    "MCP client smoke omitted reason output"
  require_pattern docs/mcp-client-smoke.md \
    'suggested tool is a usable MCP call' \
    "MCP client smoke execution-plan suggested tool contract"
  require_pattern docs/mcp-client-smoke.md \
    'protocol-level shortcut mirrors the first reading-plan row' \
    "MCP client smoke current reading step mirror contract"
  require_pattern docs/mcp-client-smoke.md \
    'candidate-ranking and continuation evidence' \
    "MCP client smoke candidate evidence contract"
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
    'Treat `context_pack\.reading_plan\[\]\.focus` as the compact scan label' \
    "MCP client config reading focus guidance"
  require_pattern docs/mcp-client-config.md \
    'If `context_pack\.continuation_summary\.status` is `blocked_no_seed`, ask the' \
    "MCP client config blocked no-seed minimal client guidance"
  require_pattern docs/mcp-client-config.md \
    'broad repository reads' \
    "MCP client config blocked no-seed broad-read guard"
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
    'first reading selection rank, current-reading-step' \
    "MCP client config first-call selection rank field"
  require_pattern docs/mcp-client-config.md \
    'mirror check, `context_pack_read_less`, `reading_plan\[\]`, continuation summary' \
    "MCP client config first-call continuation fields"
  require_pattern docs/mcp-client-config.md \
    '"context_pack_read_less": \{' \
    "MCP client config first-call read-less object"
  require_pattern docs/mcp-client-config.md \
    'Expected summary shape:' \
    "MCP client config first-call summary example"
  require_pattern docs/mcp-client-config.md \
    '"selected_files": \["src/auth\.ts", "src/audit\.ts"\]' \
    "MCP client config first-call selected files example"
  require_pattern docs/mcp-client-config.md \
    '"seed_strategy": "auto_task_path"' \
    "MCP client config first-call seed strategy example"
  require_pattern docs/mcp-client-config.md \
    '"use_current_reading_step_suggested_tool"' \
    "MCP client config first-call execution action example"
  require_pattern docs/mcp-client-config.md \
    '"first_execution_instruction_has_focus": true' \
    "MCP client config first execution focus contract example"
  require_pattern docs/mcp-client-config.md \
    '"current_reading_step_matches_reading_plan": true' \
    "MCP client config current reading step mirror example"
  require_pattern docs/mcp-client-config.md \
    '"focus": "Start with seed file authentication' \
    "MCP client config reading focus example"
  require_pattern docs/mcp-client-config.md \
    '"current_step_instruction_has_focus": true' \
    "MCP client config current-step focus contract example"
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
    '\| `agent_route\.current_reading_step` \| Mirrors `context_pack\.reading_plan\[0\]`' \
    "MCP client config current reading step signal"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.reading_plan\[\]\.focus` \| Gives the compact scan label' \
    "MCP client config reading focus signal"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.continuation_summary\.status` \| Can be `blocked_no_seed` when no source seed can be inferred\.' \
    "MCP client config blocked no-seed signal"
  require_pattern docs/mcp-client-config.md \
    '\| `execution_plan\[\]` \| Starts with `read_selected_context`, then gates deeper tools and continuation\.' \
    "MCP client config execution plan signal"
  require_pattern docs/client-workflow.md \
    'The first call is healthy when the response has either selected context or an' \
    "client workflow first-call health checklist"
  require_pattern docs/client-workflow.md \
    '`execution_plan\[0\]\.action` set to `read_selected_context`' \
    "client workflow first execution action health check"
  require_pattern docs/client-workflow.md \
    '`context_pack\.read_less` for first-read source-line reduction evidence' \
    "client workflow read-less health check"
  require_pattern docs/client-workflow.md \
    'run or report the step'"'"'s `suggested_checks\[\]`' \
    "client workflow impact execution suggested checks"
  require_pattern docs/client-workflow.md \
    '`context_pack\.continuation_summary\.status` set to `blocked_no_seed`' \
    "client workflow blocked no-seed health check"
  require_pattern docs/first-read-workflow.md \
    '`context_pack\.seed_strategy` is `auto_no_seed`' \
    "first-read workflow blocked no-seed contract"
  require_pattern docs/client-integration-examples.md \
    'Show context_pack.read_less as first-read source-line reduction evidence' \
    "client integration read-less consumption"
  require_pattern docs/client-integration-examples.md \
    'If continuation_summary.status is blocked_no_seed, ask for a seed file or' \
    "client integration blocked no-seed policy"
  require_pattern docs/agent-prompt-template.md \
    'context_pack.read_less only as read-less reporting evidence' \
    "agent prompt minimal read-less policy"
  require_pattern docs/agent-prompt-template.md \
    'If continuation_summary.status is blocked_no_seed, ask' \
    "agent prompt blocked no-seed policy"
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
    'current_reading_step_contract' \
    "two-minute demo current reading step contract metric"
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
    'agent_route\.current_reading_step mirrors reading_plan\[0\]' \
    "two-minute demo current reading step contract talk track"
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
    'Blind first-read baseline: \$\{total_lines\} source lines' \
    "two-minute demo blind baseline evidence summary"
  require_pattern scripts/two-minute-demo.sh \
    'Routed first-read: \$\{selected_lines\} source lines across \$\{selected_files\} files' \
    "two-minute demo routed first-read evidence summary"
  require_pattern scripts/two-minute-demo.sh \
    'Read less: avoided \$\{avoided_lines\} source lines, \$\{read_less\} less text before follow-up tools' \
    "two-minute demo read-less evidence summary"
  require_pattern scripts/two-minute-demo.sh \
    'agent_route selected \$\{selected_lines\}/\$\{total_lines\} source lines' \
    "two-minute demo line-reduction evidence summary"
  require_pattern scripts/two-minute-demo.sh \
    'impact_analysis reports' \
    "two-minute demo impact-analysis talk track"

  echo "docs benchmark smoke passed"
}

main "$@"
