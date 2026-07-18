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

main() {
  require_pattern docs/demo-output.md \
    '^# Two-Minute Demo Output Snapshot$' \
    "demo output title"
  require_pattern docs/demo-output.md \
    'scripts/two-minute-demo\.sh' \
    "two-minute demo command"
  require_pattern docs/demo-output.md \
    'Problem: AI agents waste the first read' \
    "problem statement"
  require_pattern docs/demo-output.md \
    'Promise: route the agent through agent_route before edits\.' \
    "product promise"
  require_pattern docs/demo-output.md \
    'CodeInsight agent_route demo' \
    "agent route live heading"
  require_pattern docs/demo-output.md \
    '1\. index_project' \
    "index stage"
  require_pattern docs/demo-output.md \
    '2\. project_overview' \
    "overview stage"
  require_pattern docs/demo-output.md \
    '3\. context_pack' \
    "context-pack stage"
  require_pattern docs/demo-output.md \
    '4\. impact_analysis' \
    "impact-analysis stage"
  require_pattern docs/demo-output.md \
    'reading_plan_steps: [0-9]+' \
    "reading-plan metric"
  require_pattern docs/demo-output.md \
    'execution_plan_steps: 4' \
    "execution-plan metric"
  require_pattern docs/demo-output.md \
    'first_execution_action: read_selected_context' \
    "first execution action metric"
  require_pattern docs/demo-output.md \
    'second_execution_action: use_current_reading_step_suggested_tool' \
    "second execution action metric"
  require_pattern docs/demo-output.md \
    'first_execution_suggested_tool: file_outline' \
    "first execution suggested tool metric"
  require_pattern docs/demo-output.md \
    'first_next_action: inspect_seed_file' \
    "first next action metric"
  require_pattern docs/demo-output.md \
    'first_reading_question: What entrypoints' \
    "first reading question metric"
  require_pattern docs/demo-output.md \
    'first_reading_file: ' \
    "first reading file metric"
  require_pattern docs/demo-output.md \
    'reading_order_contract: true' \
    "reading order contract metric"
  require_pattern docs/demo-output.md \
    'suggested_tool_handoff_contract: true' \
    "suggested tool handoff contract metric"
  require_pattern docs/demo-output.md \
    'continuation_timing_contract: true' \
    "continuation timing contract metric"
  require_pattern docs/demo-output.md \
    'route_reason: selected [0-9]+ files, [0-9]+ ranges, and [0-9]+ reading-plan steps within the token budget; read .* first via inspect_seed_file' \
    "context route reason metric"
  require_pattern docs/demo-output.md \
    'reading_plan_reason: Read this step to answer:' \
    "reading plan reason metric"
  require_pattern docs/demo-output.md \
    'selection_reason: Selected for high relevance' \
    "selection reason metric"
  require_pattern docs/demo-output.md \
    'line_reduction: [0-9]+\.[0-9]%' \
    "line reduction metric"
  require_pattern docs/demo-output.md \
    'continuation: complete' \
    "continuation status"
  require_pattern docs/demo-output.md \
    '\[Talk track\]' \
    "talk track section"
  require_pattern docs/demo-output.md \
    '\[Evidence summary\]' \
    "evidence summary section"
  require_pattern docs/demo-output.md \
    'agent_route selected [0-9]+/[0-9]+ source lines \([0-9]+\.[0-9]% reduction\) across [0-9]+ files\.' \
    "evidence summary line reduction"
  require_pattern docs/demo-output.md \
    'First reading question: What entrypoints' \
    "evidence summary first reading question"
  require_pattern docs/demo-output.md \
    'reading_plan starts at ' \
    "evidence summary first reading file"
  require_pattern docs/demo-output.md \
    'Execution contract: reading_order=true, suggested_tool_handoff=true, continuation_after_selected_context=true\.' \
    "evidence summary execution contract"
  require_pattern docs/demo-output.md \
    'Read .* before offering file_outline\.' \
    "evidence summary suggested tool timing"
  require_pattern docs/demo-output.md \
    'Before edits, impact_analysis reports high risk across [0-9]+ impacted files\.' \
    "evidence summary impact check"
  require_pattern docs/demo-output.md \
    'agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call\.' \
    "agent route talk track"
  require_pattern docs/demo-output.md \
    'project_overview found' \
    "overview talk track"
  require_pattern docs/demo-output.md \
    'context_pack selected' \
    "context-pack talk track"
  require_pattern docs/demo-output.md \
    'execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool' \
    "execution plan talk track"
  require_pattern docs/demo-output.md \
    'The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read\.' \
    "execution suggested tool talk track"
  require_pattern docs/demo-output.md \
    'The first reading-plan question is: What entrypoints' \
    "reading question talk track"
  require_pattern docs/demo-output.md \
    'Reading order contract is true; execution_plan\[0\]\.files follows reading_plan\[\] order\.' \
    "reading order contract talk track"
  require_pattern docs/demo-output.md \
    'Suggested-tool handoff contract is true; execution_plan\[1\] points to the current reading step\.' \
    "suggested tool handoff contract talk track"
  require_pattern docs/demo-output.md \
    'Continuation timing contract is true; continuation is only considered after selected context is read\.' \
    "continuation timing contract talk track"
  require_pattern docs/demo-output.md \
    'The first reading-plan action is inspect_seed_file; Read this step to answer:' \
    "reading reason talk track"
  require_pattern docs/demo-output.md \
    'Selection evidence: Selected for high relevance' \
    "selection evidence talk track"
  require_pattern docs/demo-output.md \
    'impact_analysis reports' \
    "impact-analysis talk track"
  require_pattern docs/demo-output.md \
    'pre-edit impact check estimated [0-9]+ impacted files' \
    "impact route reason talk track"
  require_pattern docs/demo-output.md \
    'The evidence summary gives a compact copyable result' \
    "evidence summary check guidance"
  require_pattern docs/demo-output.md \
    '\[Agent policy\]' \
    "agent policy section"
  require_pattern docs/demo-output.md \
    'Call agent_route with root, task, and token_budget for the default first read\.' \
    "agent policy path"
  require_pattern docs/demo-output.md \
    'CODEINSIGHT_DEMO_SAVE_JSON=/tmp/codeinsight-agent-route\.json scripts/two-minute-demo\.sh' \
    "save raw agent route JSON command"

  echo "demo output smoke passed"
}

main "$@"
