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
    'routing_decision_seed_strategy: auto_task_match' \
    "routing decision seed strategy metric"
  require_pattern docs/demo-output.md \
    'routing_decision_first_seed: task_match:src/tools\.rs' \
    "routing decision first seed metric"
  require_pattern docs/demo-output.md \
    'routing_decision_first_file: src/tools\.rs' \
    "routing decision first file metric"
  require_pattern docs/demo-output.md \
    'routing_decision_first_selection_rank: [0-9]+' \
    "routing decision first selection rank metric"
  require_pattern docs/demo-output.md \
    'routing_decision_suggested_tool: file_outline' \
    "routing decision suggested tool metric"
  require_pattern docs/demo-output.md \
    'routing_decision_read_less: [0-9]+\.[0-9]%, [0-9]+\.[0-9]x' \
    "routing decision read-less metric"
  require_pattern docs/demo-output.md \
    'routing_decision_continuation: (complete|omitted_candidates_available)' \
    "routing decision continuation metric"
  require_pattern docs/demo-output.md \
    'routing_decision_impact_status: complete' \
    "routing decision impact status metric"
  require_pattern docs/demo-output.md \
    'routing_decision_quality: high \(100/100, 22 evidence signals\)' \
    "routing decision route quality metric"
  require_pattern docs/demo-output.md \
    'routing_decision_recommended_action: read_selected_context_then_use_continuation_if_needed' \
    "routing decision route quality action"
  require_pattern docs/demo-output.md \
    'first_next_action: inspect_seed_file' \
    "first next action metric"
  require_pattern docs/demo-output.md \
    'first_reading_focus: Start with seed file' \
    "first reading focus metric"
  require_pattern docs/demo-output.md \
    'first_reading_question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here\?' \
    "first reading question metric"
  require_pattern docs/demo-output.md \
    'first_selection_rank: [0-9]+' \
    "first selection rank metric"
  require_pattern docs/demo-output.md \
    'first_reading_file: ' \
    "first reading file metric"
  require_pattern docs/demo-output.md \
    'reading_order_contract: true' \
    "reading order contract metric"
  require_pattern docs/demo-output.md \
    'read_less_instruction_contract: true' \
    "read-less instruction contract metric"
  require_pattern docs/demo-output.md \
    'current_reading_step_contract: true' \
    "current reading step contract metric"
  require_pattern docs/demo-output.md \
    'suggested_tool_handoff_contract: true' \
    "suggested tool handoff contract metric"
  require_pattern docs/demo-output.md \
    'continuation_timing_contract: true' \
    "continuation timing contract metric"
  require_pattern docs/demo-output.md \
    'route_reason: selected [0-9]+ files, [0-9]+ ranges, and [0-9]+ reading-plan steps within the token budget; read .* first \(candidate rank [0-9]+\) via inspect_seed_file' \
    "context route reason metric"
  require_pattern docs/demo-output.md \
    'reading_plan_reason: Read this step to answer:' \
    "reading plan reason metric"
  require_pattern docs/demo-output.md \
    'selection_reason: Selected for high relevance' \
    "selection reason metric"
  require_pattern docs/demo-output.md \
    'blind_first_read_lines: [0-9]+' \
    "blind first-read line baseline metric"
  require_pattern docs/demo-output.md \
    'routed_first_read_lines: [0-9]+' \
    "routed first-read line metric"
  require_pattern docs/demo-output.md \
    'source_lines_avoided: [0-9]+' \
    "source lines avoided metric"
  require_pattern docs/demo-output.md \
    'line_reduction: [0-9]+\.[0-9]%' \
    "line reduction metric"
  require_pattern docs/demo-output.md \
    'read_less_ratio: [0-9]+\.[0-9]x' \
    "read-less ratio metric"
  require_pattern docs/demo-output.md \
    'continuation: (complete|omitted_candidates_available)' \
    "continuation status"
  require_pattern docs/demo-output.md \
    'continuation_next_action: (read_selected_context|run_omitted_candidate_context_pack)' \
    "continuation next action"
  require_pattern docs/demo-output.md \
    'first_omitted_candidate: (none|.+)' \
    "omitted candidate status"
  require_pattern docs/demo-output.md \
    '\[Talk track\]' \
    "talk track section"
  require_pattern docs/demo-output.md \
    '\[Evidence summary\]' \
    "evidence summary section"
  require_pattern docs/demo-output.md \
    'Blind first-read baseline: [0-9]+ source lines\.' \
    "evidence summary blind baseline"
  require_pattern docs/demo-output.md \
    'Routed first-read: [0-9]+ source lines across [0-9]+ files\.' \
    "evidence summary routed first-read"
  require_pattern docs/demo-output.md \
    'Read less: avoided [0-9]+ source lines, [0-9]+\.[0-9]x less text before follow-up tools\.' \
    "evidence summary read-less ratio"
  require_pattern docs/demo-output.md \
    'Routing decision: seed=task_match:src/tools\.rs, first_file=src/tools\.rs, rank=[0-9]+, tool=file_outline, continuation=(complete|omitted_candidates_available), impact=complete\.' \
    "evidence summary routing decision"
  require_pattern docs/demo-output.md \
    'agent_route selected [0-9]+/[0-9]+ source lines \([0-9]+\.[0-9]% reduction\) across [0-9]+ files\.' \
    "evidence summary line reduction"
  require_pattern docs/demo-output.md \
    'First reading focus: Start with seed file' \
    "evidence summary first reading focus"
  require_pattern docs/demo-output.md \
    'First reading question: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here\?' \
    "evidence summary first reading question"
  require_pattern docs/demo-output.md \
    'reading_plan starts at .* as candidate rank [0-9]+\.' \
    "evidence summary first reading rank"
  require_pattern docs/demo-output.md \
    'Execution contract: reading_order=true, read_less_instruction=true, current_reading_step=true, suggested_tool_handoff=true, continuation_after_selected_context=true\.' \
    "evidence summary execution contract"
  require_pattern docs/demo-output.md \
    'Selection evidence: Selected for high relevance' \
    "evidence summary selection evidence"
  require_pattern docs/demo-output.md \
    'Continuation: status=(complete|omitted_candidates_available), next_action=(read_selected_context|run_omitted_candidate_context_pack)\.' \
    "evidence summary continuation"
  require_pattern docs/demo-output.md \
    'Next follow-up candidate: (none before selected context is read|.+)' \
    "evidence summary omitted candidate"
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
    'routing_decision summarizes the same choice: seed=task_match:src/tools\.rs, first_file=src/tools\.rs, rank=[0-9]+, read_less=[0-9]+\.[0-9]%/[0-9]+\.[0-9]x\.' \
    "routing decision talk track"
  require_pattern docs/demo-output.md \
    'The first reading-plan focus is: Start with seed file' \
    "reading focus talk track"
  require_pattern docs/demo-output.md \
    'The first reading-plan question is: Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here\?' \
    "reading question talk track"
  require_pattern docs/demo-output.md \
    'Reading order contract is true; execution_plan\[0\]\.files follows reading_plan\[\] order\.' \
    "reading order contract talk track"
  require_pattern docs/demo-output.md \
    'Read-less instruction contract is true; execution_plan\[0\]\.instruction carries selected lines, baseline lines, avoided lines, and read-less ratio\.' \
    "read-less instruction contract talk track"
  require_pattern docs/demo-output.md \
    'Current reading step contract is true; agent_route\.current_reading_step mirrors reading_plan\[0\]\.' \
    "current reading step contract talk track"
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
    'Selection evidence: candidate rank [0-9]+; Selected for high relevance' \
    "selection evidence talk track"
  require_pattern docs/demo-output.md \
    'Continuation status is (complete; next_action=read_selected_context, so no omitted candidate follow-up is needed before selected context is read|omitted_candidates_available; next follow-up is .* next_action=run_omitted_candidate_context_pack)\.' \
    "continuation next action talk track"
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
