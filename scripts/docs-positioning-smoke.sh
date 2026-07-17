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
  require_pattern README.md \
    'local-first code context router|local-first MCP code context router|local-first MCP code context routing' \
    "local-first AI-agent positioning"
  require_pattern README.md \
    'agent_route -> selected context -> executable suggested_tool -> impact check' \
    "README first-read route loop"
  require_pattern README.md \
    '`execution_plan\[\]` actions that keep focused follow-up tools behind the' \
    "README execution-plan gating"
  require_pattern README.md \
    'an executable `suggested_tool` such as `file_outline`' \
    "README executable suggested tool positioning"
  require_pattern README.md \
    'CodeInsight is not trying to replace an IDE, LSP, compiler, or Sourcegraph' \
    "explicit non-goal positioning"
  require_pattern README.md \
    '^## Fast Path$' \
    "README fast path section"
  require_pattern README.md \
    'Add the local MCP server' \
    "README fast path MCP setup"
  require_pattern README.md \
    '\[MCP client configuration\]\(docs/mcp-client-config\.md\)' \
    "README fast path MCP config link"
  require_pattern README.md \
    'Call agent_route with root, task, and token_budget 6000 before reading files directly' \
    "README fast path agent_route prompt"
  require_pattern README.md \
    'Follow agent_route\.execution_plan\[\] in order' \
    "README fast path execution plan policy"
  require_pattern README.md \
    '\[First Agent Route Call\]\(docs/mcp-client-config\.md#first-agent-route-call\)' \
    "README first agent route call link"
  require_pattern README.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh' \
    "README fast path installed quickstart gate"
  require_pattern README.md \
    '\[Evidence summary\]' \
    "README fast path evidence summary cue"
  require_pattern README.md \
    '\[First-read workflow\]\(docs/first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern README.md \
    '\[Known limitations\]\(docs/known-limitations\.md\)' \
    "known limitations link"
  require_pattern README.md \
    '\[Client integration examples\]\(docs/client-integration-examples\.md\)' \
    "client integration examples link"
  require_pattern README.md \
    'scripts/installed-quickstart-smoke\.sh' \
    "README installed quickstart adoption gate"
  require_pattern README.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "README CLI and MCP agent_route coverage"
  require_pattern README.md \
    '`agent_route`, which runs `index_project -> project_overview -> context_pack ->' \
    "README one-call agent_route demo path"
  require_pattern README.md \
    'reading-plan reasons, selection evidence' \
    "README demo reading reason evidence"
  require_pattern README.md \
    'the first executable suggested tool' \
    "README demo executable suggested tool evidence"
  require_pattern README.md \
    'why the agent should read it first' \
    "README first-read reason positioning"
  require_pattern README.md \
    'when a local tool is safe to' \
    "README suggested tool timing"
  require_pattern README.md \
    'Context reading plan' \
    "README benchmark reading plan evidence"
  require_pattern README.md \
    'two-minute demo for this repository shows the agent route selecting' \
    "README demo evidence summary"
  require_pattern README.md \
    '`file_outline` behind the selected-context read before the impact check' \
    "README demo suggested tool gating evidence"
  require_pattern README.md \
    'executes `agent_route\.execution_plan\[\]\.suggested_tool`' \
    "README MCP suggested tool execution evidence"

  require_pattern docs/README.md \
    '\[First-read workflow\]\(first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern docs/README.md \
    '\[Known limitations\]\(known-limitations\.md\)' \
    "known limitations link"
  require_pattern docs/README.md \
    '\[Client integration examples\]\(client-integration-examples\.md\)' \
    "docs index client integration examples link"
  require_pattern docs/README.md \
    'Default adoption path:' \
    "docs index default adoption path"
  require_pattern docs/README.md \
    'Configure a local stdio MCP server with' \
    "docs index MCP config adoption path"
  require_pattern docs/README.md \
    'Use `agent_route` as the default first-read route' \
    "docs index agent_route adoption path"
  require_pattern docs/README.md \
    'Installed first-read route: `scripts/installed-quickstart-smoke\.sh`' \
    "docs index installed quickstart validation"
  require_pattern docs/README.md \
    'Choose the check by adoption stage:' \
    "docs index validation chooser"
  require_pattern docs/README.md \
    '\| Product walkthrough \| `scripts/two-minute-demo\.sh` \|' \
    "docs index product walkthrough validation"
  require_pattern docs/README.md \
    '\| MCP client wiring \| `scripts/mcp-stdio-smoke\.sh` \|' \
    "docs index MCP wiring validation"
  require_pattern docs/README.md \
    '\| Installed-binary adoption gate \| `scripts/installed-quickstart-smoke\.sh` \|' \
    "docs index installed adoption validation"

  require_pattern docs/quickstart.md \
    'local-first' \
    "local-first setup framing"
  require_pattern docs/quickstart.md \
    '^## Fast Path$' \
    "quickstart fast path section"
  require_pattern docs/quickstart.md \
    'Configure the local stdio MCP server' \
    "quickstart MCP setup fast path"
  require_pattern docs/quickstart.md \
    'broad repository tasks start with `agent_route`' \
    "quickstart agent_route fast path"
  require_pattern docs/quickstart.md \
    '`scripts/two-minute-demo\.sh` for a visible evidence summary' \
    "quickstart demo evidence fast path"
  require_pattern docs/quickstart.md \
    '^## 5\. Choose A Smoke Check$' \
    "quickstart smoke chooser section"
  require_pattern docs/quickstart.md \
    '\| You want a visible product walkthrough \| `scripts/two-minute-demo\.sh` \|' \
    "quickstart product walkthrough chooser"
  require_pattern docs/quickstart.md \
    '\| You are wiring an MCP client from this checkout \| `scripts/mcp-stdio-smoke\.sh` \|' \
    "quickstart MCP smoke chooser"
  require_pattern docs/quickstart.md \
    '\| You installed `codeinsight` and want an adoption gate \| `CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh` \|' \
    "quickstart installed smoke chooser"
  require_pattern docs/quickstart.md \
    '\[Client workflow\]\(client-workflow\.md#agent-policy-prompt\)' \
    "agent policy prompt link"
  require_pattern docs/quickstart.md \
    'Call agent_route with root, task, and token_budget' \
    "quickstart agent_route policy"
  require_pattern docs/quickstart.md \
    '\[Adoption checklist\]\(adoption-checklist\.md\)' \
    "adoption checklist link"
  require_pattern docs/quickstart.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh' \
    "quickstart installed quickstart binary command"
  require_pattern docs/quickstart.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "quickstart installed agent_route coverage"

  require_pattern docs/cli-usage.md \
    '\[First-read workflow\]\(first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern docs/cli-usage.md \
    '\[MCP client smoke test\]\(mcp-client-smoke\.md\)' \
    "MCP smoke link"

  require_pattern docs/mcp-tools.md \
    '\[First-read workflow\]\(first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern docs/mcp-tools.md \
    '\[Known limitations\]\(known-limitations\.md\)' \
    "known limitations link"
  require_pattern docs/mcp-tools.md \
    '`agent_route` \| Run the default first-read path.*include `execution_plan\[\]`' \
    "MCP tools agent_route execution plan"
  require_pattern docs/mcp-tools.md \
    'Follow `agent_route\.execution_plan\[\]`' \
    "MCP tools execution plan first-read guidance"
  require_pattern docs/client-workflow.md \
    'Call `agent_route` with `root`, `task`, and `token_budget`' \
    "client workflow agent_route path"
  require_pattern docs/client-workflow.md \
    '^## First Agent Route Call$' \
    "client workflow first agent route call section"
  require_pattern docs/client-workflow.md \
    '"name": "agent_route"' \
    "client workflow first agent route JSON"
  require_pattern docs/client-workflow.md \
    'Use `agent_route\.execution_plan\[\]` as the client checklist' \
    "client workflow execution plan checklist"
  require_pattern docs/client-workflow.md \
    '\[Client integration examples\]\(client-integration-examples\.md\)' \
    "client workflow integration examples link"
  require_pattern docs/client-workflow.md \
    'Treat `reading_plan\[\]\.reason` as the instruction for' \
    "client workflow reading reason instruction"
  require_pattern docs/client-workflow.md \
    '`reading_plan\[\]\.selection_reason` as the compact' \
    "client workflow selection reason evidence"
  require_pattern docs/client-workflow.md \
    'Continuation actions should wait until the selected `files\[\]` excerpts' \
    "client workflow continuation ordering"
  require_pattern docs/agent-prompt-template.md \
    'call agent_route with root, task, and' \
    "agent prompt agent_route path"
  require_pattern docs/agent-prompt-template.md \
    '\[Client integration examples\]\(client-integration-examples\.md\)' \
    "agent prompt integration examples link"
  require_pattern docs/agent-prompt-template.md \
    'Treat reading_plan\.reason as' \
    "agent prompt reading reason policy"
  require_pattern docs/first-read-workflow.md \
    '`agent_route` is the default first-read contract' \
    "first-read workflow agent_route contract"
  require_pattern docs/first-read-workflow.md \
    'Use `reason` as the agent-facing instruction' \
    "first-read workflow reason instruction"
  require_pattern docs/first-read-workflow.md \
    'Use `selection_reason` for compact UI labels' \
    "first-read workflow selection reason UI guidance"
  require_pattern docs/mcp-client-config.md \
    'Call `agent_route` with `root`, `task`, and `token_budget`' \
    "MCP client config agent_route flow"
  require_pattern docs/mcp-client-config.md \
    '^## First Agent Route Call$' \
    "MCP client config first agent route section"
  require_pattern docs/mcp-client-config.md \
    '"name": "agent_route"' \
    "MCP client config first agent route JSON"
  require_pattern docs/mcp-client-config.md \
    'Use `scripts/mcp-stdio-smoke\.sh` to verify this path end to end' \
    "MCP client config smoke verification"
  require_pattern docs/mcp-client-config.md \
    '\[Client integration examples\]\(client-integration-examples\.md\)' \
    "MCP client config integration examples link"
  require_pattern docs/mcp-client-config.md \
    '`reason` is the executable instruction for' \
    "MCP client config reason contract"
  require_pattern docs/mcp-client-config.md \
    '`selection_reason` is the compact raw ranking reason' \
    "MCP client config selection reason contract"
  require_pattern docs/mcp-client-config.md \
    'Expected first-call signals:' \
    "MCP client config first-call signal table"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.files\[\]` \| Contains the bounded files or excerpts to read first\.' \
    "MCP client config bounded context signal"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.reading_plan\[\]\.reason` \| Explains what the agent should learn' \
    "MCP client config reading reason signal"
  require_pattern docs/mcp-client-config.md \
    '\| `execution_plan\[\]` \| Starts with `read_selected_context`' \
    "MCP client config execution plan signal"
  require_pattern docs/mcp-client-config.md \
    '\| `impact_status` \| Usually `complete` when a seed file or symbol was selected\.' \
    "MCP client config impact signal"
  require_pattern docs/recommendation-contract.md \
    '## Agent Route Execution Plan' \
    "recommendation contract execution plan section"
  require_pattern docs/recommendation-contract.md \
    '`agent_route\.execution_plan\[\]` is the machine-readable action sequence' \
    "recommendation contract execution plan definition"
  require_pattern docs/recommendation-contract.md \
    'The default action order is:' \
    "recommendation contract execution plan order"
  require_pattern docs/recommendation-contract.md \
    '`review_impact_before_edits`' \
    "recommendation contract impact checkpoint"
  require_pattern docs/adoption-checklist.md \
    'scripts/installed-quickstart-smoke\.sh' \
    "adoption installed quickstart gate"
  require_pattern docs/adoption-checklist.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "adoption CLI and MCP agent_route coverage"
  require_pattern docs/adoption-checklist.md \
    'The agent calls `agent_route` with `root`, `task`, and `token_budget` before' \
    "adoption agent_route first-read policy"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[\]\.reason` as the current-step instruction' \
    "adoption reading reason policy"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[\]\.selection_reason` as the evidence' \
    "adoption selection reason evidence"
  require_pattern docs/adoption-checklist.md \
    'does not execute `continuation_summary\.suggested_tool`' \
    "adoption continuation ordering"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[0\]\.reason` is present' \
    "adoption context-pack reason gate"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[0\]\.selection_reason` is present' \
    "adoption context-pack selection reason gate"

  require_pattern docs/client-integration-examples.md \
    'Every client should treat `agent_route\.execution_plan\[\]` as the ordered action' \
    "client examples execution plan contract"
  require_pattern docs/client-integration-examples.md \
    'read_selected_context -> use_current_reading_step_suggested_tool ->' \
    "client examples action order"
  require_pattern docs/client-integration-examples.md \
    'Only offer continuation_summary\.suggested_tool after selected context has been' \
    "client examples continuation ordering"
  require_pattern docs/client-integration-examples.md \
    'Suggested-tool buttons should be disabled or visually secondary until the' \
    "client examples suggested tool gating"
  require_pattern docs/client-integration-examples.md \
    'Codex' \
    "client examples Codex section"
  require_pattern docs/client-integration-examples.md \
    'Claude Code' \
    "client examples Claude Code section"
  require_pattern docs/client-integration-examples.md \
    'Cursor' \
    "client examples Cursor section"

  require_pattern docs/maintainer-checklist.md \
    'local-first MCP code context routing' \
    "maintainer focus statement"
  require_pattern docs/maintainer-checklist.md \
    '\[Known limitations\]\(known-limitations\.md\)' \
    "known limitations link"

  echo "docs positioning smoke passed"
}

main "$@"
