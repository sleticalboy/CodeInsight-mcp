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
    'local-first first-read router|local-first code context router|local-first MCP code context router|local-first MCP code context routing' \
    "local-first AI-agent positioning"
  require_pattern README.md \
    'local-first first-read router for AI coding agents' \
    "README first-read router positioning"
  require_pattern README.md \
    'turns "scan the repository and guess what matters" into a bounded local route' \
    "README bounded local route positioning"
  require_pattern README.md \
    'The product is intentionally narrow' \
    "README intentionally narrow scope"
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
    'LSP, compiler, test runner, and language-specific tools' \
    "README keep using precise local tools"
  require_pattern README.md \
    '^Best-fit tasks:$' \
    "README best-fit task boundary"
  require_pattern README.md \
    'understand a new repository before opening many files' \
    "README repository first-read use case"
  require_pattern README.md \
    'Do not treat CodeInsight output as compiler-grade proof' \
    "README compiler-grade non-goal"
  require_pattern README.md \
    'router: it narrows the first read' \
    "README context-router boundary"
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
    'Treat reading_plan\.question as the local checklist for the selected file' \
    "README fast path reading question prompt"
  require_pattern README.md \
    'Read selected files in reading_plan order and use selection_rank as the audit trail' \
    "README fast path selection rank prompt"
  require_pattern README.md \
    'Use continuation_summary only after selected context is consumed' \
    "README fast path continuation prompt"
  require_pattern README.md \
    'Follow agent_route\.execution_plan\[\] in order' \
    "README fast path execution plan policy"
  require_pattern README.md \
    '\[First Agent Route Call\]\(docs/mcp-client-config\.md#first-agent-route-call\)' \
    "README first agent route call link"
  require_pattern README.md \
    'Pick the validation that matches your adoption stage:' \
    "README fast path validation chooser"
  require_pattern README.md \
    '\| First look \| `scripts/two-minute-demo\.sh` \|' \
    "README fast path product demo validation"
  require_pattern README.md \
    '\| MCP wiring \| `CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/mcp-first-call-smoke\.sh` \|' \
    "README fast path MCP first-call validation"
  require_pattern README.md \
    '\| Installed adoption \| `CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh` \|' \
    "README fast path installed adoption validation"
  require_pattern README.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh' \
    "README fast path installed quickstart gate"
  require_pattern README.md \
    'compact JSON proof that stdio MCP accepts `agent_route`' \
    "README fast path MCP first-call proof"
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
    'reading-plan questions, reading-plan reasons, candidate selection rank' \
    "README demo candidate rank evidence"
  require_pattern README.md \
    'what question it should answer' \
    "README demo reading question positioning"
  require_pattern README.md \
    'executable suggested tool, continuation status' \
    "README demo executable suggested tool evidence"
  require_pattern README.md \
    'should read it first' \
    "README first-read reason positioning"
  require_pattern README.md \
    'when a local tool is safe to' \
    "README suggested tool timing"
  require_pattern README.md \
    'Context reading plan' \
    "README benchmark reading plan evidence"
  require_pattern README.md \
    'rank, the local reading question, first-read reason' \
    "README benchmark reading rank evidence"
  require_pattern README.md \
    'two-minute demo for this repository shows the agent route selecting' \
    "README demo evidence summary"
  require_pattern README.md \
    '`file_outline` behind the selected-context read' \
    "README demo suggested tool gating evidence"
  require_pattern README.md \
    'mirroring `current_reading_step` to `reading_plan\[0\]`' \
    "README demo current reading step evidence"
  require_pattern README.md \
    'continuation status before the impact check' \
    "README demo continuation evidence"
  require_pattern README.md \
    'executes `agent_route\.execution_plan\[\]\.suggested_tool`' \
    "README MCP suggested tool execution evidence"
  require_pattern README.md \
    'MCP first-call contract: reading_order=`true`, current_reading_step=`true`, suggested_tool_handoff=`true`, continuation_after_selected_context=`true`' \
    "README MCP first-call contract evidence"
  require_pattern README.md \
    'Use `agent_route\.current_reading_step` as the first checklist row' \
    "README MCP tools current reading step flow"
  require_pattern README.md \
    '\[CodeInsight self adoption report\]\(docs/adoption-report-codeinsight\.md\)' \
    "README self adoption report evidence"

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
    '\| Copyable MCP first-call JSON \| `scripts/mcp-first-call-smoke\.sh` \|' \
    "docs index MCP first-call validation"
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
    'The main path is intentionally one call from the agent: `agent_route`' \
    "quickstart one-call agent route framing"
  require_pattern docs/quickstart.md \
    'refreshes the local index, returns a repository overview, selects the bounded' \
    "quickstart one-call route contents"
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
    'The demo calls `agent_route`' \
    "quickstart demo agent_route call"
  require_pattern docs/quickstart.md \
    'The returned `route\[\]` records the local work' \
    "quickstart route provenance"
  require_pattern docs/quickstart.md \
    '^## 5\. Choose A Smoke Check$' \
    "quickstart smoke chooser section"
  require_pattern docs/quickstart.md \
    '\| You want a visible product walkthrough \| `scripts/two-minute-demo\.sh` \|' \
    "quickstart product walkthrough chooser"
  require_pattern docs/quickstart.md \
    '\| You want a copyable first MCP call summary \| `scripts/mcp-first-call-smoke\.sh` \|' \
    "quickstart MCP first-call chooser"
  require_pattern docs/quickstart.md \
    'Expected output shape:' \
    "quickstart MCP first-call output shape"
  require_pattern docs/quickstart.md \
    'scripts/mcp-first-call-smoke\.sh --help' \
    "quickstart MCP first-call help"
  require_pattern docs/quickstart.md \
    'scripts/mcp-first-call-smoke\.sh --summary-json /tmp/codeinsight-mcp-first-call\.json' \
    "quickstart MCP first-call summary JSON"
  require_pattern docs/quickstart.md \
    '"route_tools": \[' \
    "quickstart MCP first-call route tools"
  require_pattern docs/quickstart.md \
    '"reading_plan": \[' \
    "quickstart MCP first-call reading plan"
  require_pattern docs/quickstart.md \
    '"first_context_file": "src/main\.ts"' \
    "quickstart MCP first-call first context file"
  require_pattern docs/quickstart.md \
    '"first_reading_file": "src/main\.ts"' \
    "quickstart MCP first-call first reading file"
  require_pattern docs/quickstart.md \
    '"next_action": "inspect_seed_file"' \
    "quickstart MCP first-call next action"
  require_pattern docs/quickstart.md \
    '"current_reading_step_matches_reading_plan": true' \
    "quickstart MCP first-call current reading step mirror"
  require_pattern docs/quickstart.md \
    '"focus": "Start with seed file context' \
    "quickstart MCP first-call reading focus"
  require_pattern docs/quickstart.md \
    '"question": "What entrypoints' \
    "quickstart MCP first-call reading question"
  require_pattern docs/quickstart.md \
    '"execution_plan_reads_in_reading_plan_order": true' \
    "quickstart MCP first-call reading order contract"
  require_pattern docs/quickstart.md \
    '"first_execution_instruction_has_focus": true' \
    "quickstart MCP first-call first execution focus contract"
  require_pattern docs/quickstart.md \
    '"first_execution_instruction_has_question": true' \
    "quickstart MCP first-call first execution question contract"
  require_pattern docs/quickstart.md \
    '"current_step_suggested_tool_matches_reading_plan": true' \
    "quickstart MCP first-call suggested tool contract"
  require_pattern docs/quickstart.md \
    '"current_step_instruction_has_focus": true' \
    "quickstart MCP first-call current-step focus contract"
  require_pattern docs/quickstart.md \
    '"current_step_instruction_has_question": true' \
    "quickstart MCP first-call current-step question contract"
  require_pattern docs/quickstart.md \
    '"continuation_after_selected_context": true' \
    "quickstart MCP first-call continuation contract"
  require_pattern docs/quickstart.md \
    '"suggested_tool_executed": true' \
    "quickstart MCP first-call suggested tool execution"
  require_pattern docs/quickstart.md \
    '"impact_status": "complete"' \
    "quickstart MCP first-call impact status"
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
    'Use reading_plan\.focus as the compact scan label' \
    "quickstart reading focus policy"
  require_pattern docs/quickstart.md \
    '\[Adoption checklist\]\(adoption-checklist\.md\)' \
    "adoption checklist link"
  require_pattern docs/quickstart.md \
    'CODEINSIGHT_BIN="\$\(command -v codeinsight\)" scripts/installed-quickstart-smoke\.sh' \
    "quickstart installed quickstart binary command"
  require_pattern docs/quickstart.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "quickstart installed agent_route coverage"
  require_pattern docs/quickstart.md \
    '`reading_plan\.focus`, `reading_plan\.question`, `reading_plan\.reason`,' \
    "quickstart installed reading focus coverage"
  require_pattern docs/quickstart.md \
    '`selection_rank`, and continuation evidence' \
    "quickstart installed rank continuation coverage"
  require_pattern docs/product-prototype.md \
    '本地优先的 AI Agent 代码上下文路由器' \
    "product prototype context router positioning"
  require_pattern docs/product-prototype.md \
    '不替代 IDE、LSP、编译器、测试框架或 Sourcegraph' \
    "product prototype non-replacement boundary"
  require_pattern docs/product-prototype.md \
    'agent_route -> selected context -> suggested_tool -> impact check' \
    "product prototype first-read route"
  require_pattern docs/product-prototype.md \
    '当前 MVP 支持这些语法级基础索引语言' \
    "product prototype current MVP language scope"
  require_pattern docs/product-prototype.md \
    '精度目标是 Agent 导航可用，不承诺 compiler-grade 或 LSP-grade' \
    "product prototype accuracy boundary"
  require_pattern docs/product-prototype.md \
    '### 5\.11 `agent_route`' \
    "product prototype agent_route tool"
  require_pattern docs/product-prototype.md \
    '真正核心是 `agent_route`、`context_pack`、阅读计划、后续工具交接和修改前影响预览' \
    "product prototype semantic-search non-core boundary"
  require_pattern docs/implementation-plan.md \
    '通过 `agent_route` 给 AI Agent 提供 first-read 路线' \
    "implementation plan agent_route strategy"
  require_pattern docs/implementation-plan.md \
    '当前 MVP 目标已经从“能否做出来”转为“能否被真实 Agent 工作流持续采用”' \
    "implementation plan current MVP stage"
  require_pattern docs/implementation-plan.md \
    '\| `agent_route` \| P0 \| 默认 first-read 入口 \|' \
    "implementation plan agent_route P0"
  require_pattern docs/implementation-plan.md \
    '以下能力不作为最小 MVP 的默认主线' \
    "implementation plan default MVP boundary"
  require_pattern docs/implementation-plan.md \
    '当前不把商业化作为主线' \
    "implementation plan commercialization boundary"

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
    '`agent_route` \| Run the default first-read path.*expose `current_reading_step` and `execution_plan\[\]`' \
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
    'Use `agent_route\.current_reading_step` as the first' \
    "client workflow current reading step policy"
  require_pattern docs/client-workflow.md \
    'Treat `reading_plan\[\]\.focus` as the compact scan label' \
    "client workflow reading focus policy"
  require_pattern docs/client-workflow.md \
    '`reading_plan\[\]\.question` as the local checklist' \
    "client workflow reading question checklist"
  require_pattern docs/client-workflow.md \
    '`reading_plan\[\]\.reason` as the instruction' \
    "client workflow reading reason instruction"
  require_pattern docs/client-workflow.md \
    '`reading_plan\[\]\.selection_rank` as the candidate rank' \
    "client workflow selection rank evidence"
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
    'Treat reading_plan\.focus as' \
    "agent prompt reading focus policy"
  require_pattern docs/agent-prompt-template.md \
    'reading_plan\.reason as the current-step instruction' \
    "agent prompt reading reason policy"
  require_pattern docs/agent-prompt-template.md \
    'reading_plan\.selection_rank as the candidate rank audit trail' \
    "agent prompt selection rank policy"
  require_pattern docs/agent-prompt-template.md \
    'Use each reading_plan\.question as the local checklist' \
    "agent prompt first read question policy"
  require_pattern docs/agent-prompt-template.md \
    'Use continuation_summary\.next_action to decide whether to narrow the task' \
    "agent prompt continuation next action policy"
  require_pattern docs/agent-prompt-template.md \
    'omitted_candidates\[\]\.selection_rank and omission_reason' \
    "agent prompt omitted candidate rank policy"
  require_pattern docs/agent-prompt-template.md \
    'Do not use suggested_tool or continuation as a shortcut around reading the' \
    "agent prompt suggested tool ordering"
  require_pattern docs/agent-prompt-template.md \
    'Read selected files in reading_plan order before' \
    "agent prompt minimal ordering"
  require_pattern docs/first-read-workflow.md \
    '`agent_route` is the default first-read contract' \
    "first-read workflow agent_route contract"
  require_pattern docs/first-read-workflow.md \
    'agent_route -> read selected context -> current-step suggested_tool if needed -> continuation if needed -> impact check before edits' \
    "first-read workflow ordering contract"
  require_pattern docs/first-read-workflow.md \
    '`suggested_tool` and continuation are follow-up actions, not replacements' \
    "first-read workflow suggested tool boundary"
  require_pattern docs/first-read-workflow.md \
    '`impact_analysis` is the pre-edit planning gate' \
    "first-read workflow impact gate"
  require_pattern docs/first-read-workflow.md \
    'Use `agent_route\.current_reading_step` as the first checklist' \
    "first-read workflow current reading step"
  require_pattern docs/first-read-workflow.md \
    'Treat `reading_plan\[\]\.focus` as the compact scan label' \
    "first-read workflow reading focus"
  require_pattern docs/first-read-workflow.md \
    'Use `question` as the local checklist' \
    "first-read workflow question checklist"
  require_pattern docs/first-read-workflow.md \
    'Use `reason` as the agent-facing instruction' \
    "first-read workflow reason instruction"
  require_pattern docs/first-read-workflow.md \
    'Use `selection_reason` for compact UI labels' \
    "first-read workflow selection reason UI guidance"
  require_pattern docs/client-workflow.md \
    '^Client invariants:$' \
    "client workflow invariants section"
  require_pattern docs/client-workflow.md \
    'Never run a `reading_plan\[\]\.suggested_tool` before reading the matching' \
    "client workflow suggested tool invariant"
  require_pattern docs/client-workflow.md \
    'Never run `continuation_summary\.suggested_tool` before the selected context' \
    "client workflow continuation invariant"
  require_pattern docs/client-workflow.md \
    'Always treat `impact_analysis` as the pre-edit planning gate' \
    "client workflow impact invariant"
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
    'For a shorter copyable check, run `scripts/mcp-first-call-smoke\.sh`' \
    "MCP client config first-call smoke"
  require_pattern docs/mcp-client-config.md \
    'Expected summary shape:' \
    "MCP client config first-call summary shape"
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
    'Treat `context_pack\.reading_plan\[\]\.focus` as the compact scan label' \
    "MCP client config reading focus client action"
  require_pattern docs/mcp-client-config.md \
    '`reading_plan\[\]\.question` as the local checklist' \
    "MCP client config agent policy reading question"
  require_pattern docs/mcp-client-config.md \
    'Expected first-call signals:' \
    "MCP client config first-call signal table"
  require_pattern docs/mcp-client-config.md \
    '^UI gating rules:$' \
    "MCP client config UI gating section"
  require_pattern docs/mcp-client-config.md \
    'Keep `execution_plan\[\]\.suggested_tool` disabled or visually secondary until' \
    "MCP client config suggested tool UI gate"
  require_pattern docs/mcp-client-config.md \
    'Keep `continuation_summary\.suggested_tool` hidden, disabled, or visually' \
    "MCP client config continuation UI gate"
  require_pattern docs/mcp-client-config.md \
    'Surface `impact_analysis` as the pre-edit review step' \
    "MCP client config impact UI gate"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.files\[\]` \| Contains the bounded files or excerpts to read first\.' \
    "MCP client config bounded context signal"
  require_pattern docs/mcp-client-config.md \
    '\| `context_pack\.reading_plan\[\]\.question` \| States the concrete question' \
    "MCP client config reading question signal"
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
    'first reading focus/question evidence' \
    "adoption installed quickstart reading focus output"
  require_pattern docs/adoption-checklist.md \
    '`context_reading_question`' \
    "adoption installed quickstart reading question output"
  require_pattern docs/adoption-checklist.md \
    '`mcp_agent_route_reading_question`' \
    "adoption installed quickstart MCP reading question output"
  require_pattern docs/adoption-checklist.md \
    '`mcp_agent_route_selection_rank`' \
    "adoption installed quickstart MCP selection rank output"
  require_pattern docs/adoption-checklist.md \
    'continuation status and next-action fields' \
    "adoption installed quickstart continuation output"
  require_pattern docs/adoption-checklist.md \
    'The agent calls `agent_route` with `root`, `task`, and `token_budget` before' \
    "adoption agent_route first-read policy"
  require_pattern docs/adoption-checklist.md \
    '`agent_route\.current_reading_step` for the first checklist row' \
    "adoption current reading step policy"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[\]\.focus` as the compact scan label' \
    "adoption reading focus policy"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[\]\.question` as the local checklist' \
    "adoption reading question policy"
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
    'Suggested-tool controls are disabled or visually secondary until the matching' \
    "adoption suggested tool UI gate"
  require_pattern docs/adoption-checklist.md \
    'Continuation controls are hidden, disabled, or visually secondary until the' \
    "adoption continuation UI gate"
  require_pattern docs/adoption-checklist.md \
    'Impact-review controls are shown before edits' \
    "adoption impact UI gate"
  require_pattern docs/adoption-checklist.md \
    'does not execute `reading_plan\[\]\.suggested_tool` until the matching' \
    "adoption suggested tool ordering"
  require_pattern docs/adoption-checklist.md \
    'without treating it as proof' \
    "adoption impact safety boundary"
  require_pattern docs/adoption-checklist.md \
    'Suggested-tool and continuation controls are gated behind selected-context' \
    "adoption complete UI gate"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[0\]\.reason` is present' \
    "adoption context-pack reason gate"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[0\]\.question` is present' \
    "adoption context-pack question gate"
  require_pattern docs/adoption-checklist.md \
    '`reading_plan\[0\]\.selection_reason` is present' \
    "adoption context-pack selection reason gate"

  require_pattern docs/client-integration-examples.md \
    'Every client should treat `agent_route\.execution_plan\[\]` as the ordered action' \
    "client examples execution plan contract"
  require_pattern docs/client-integration-examples.md \
    'Use agent_route\.current_reading_step for the first checklist row' \
    "client examples current reading step policy"
  require_pattern docs/client-integration-examples.md \
    'Use reading_plan\[\]\.focus as the compact scan label' \
    "client examples reading focus policy"
  require_pattern docs/client-integration-examples.md \
    'Use reading_plan\[\]\.question as the local checklist' \
    "client examples reading question policy"
  require_pattern docs/client-integration-examples.md \
    'Use reading_plan\[\]\.selection_rank as the candidate rank audit trail' \
    "client examples selection rank policy"
  require_pattern docs/client-integration-examples.md \
    'Use continuation_summary\.next_action and omitted_candidates\[\] to explain' \
    "client examples continuation next action policy"
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
    'active only after the matching selected context file is read' \
    "client examples suggested tool active state"
  require_pattern docs/client-integration-examples.md \
    '`continuation_summary\.suggested_tool` as a continue action only after the' \
    "client examples continuation button state"
  require_pattern docs/client-integration-examples.md \
    'Continuation buttons should stay disabled or hidden until selected context' \
    "client examples continuation disabled state"
  require_pattern docs/client-integration-examples.md \
    'Impact-review controls should be shown after the first read and before edits' \
    "client examples impact review state"
  require_pattern docs/client-integration-examples.md \
    '`agent_route\.current_reading_step` as the first checklist row' \
    "client examples UI current reading step"
  require_pattern docs/client-integration-examples.md \
    '`reading_plan\[\]\.focus` beside each selected file' \
    "client examples UI reading focus"
  require_pattern docs/client-integration-examples.md \
    '`reading_plan\[\]\.question` beside each selected file' \
    "client examples UI reading question"
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
