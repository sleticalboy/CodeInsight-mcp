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
    'local-first code context router|local-first MCP code context routing' \
    "local-first AI-agent positioning"
  require_pattern README.md \
    'CodeInsight is not trying to replace an IDE, LSP, compiler, or Sourcegraph' \
    "explicit non-goal positioning"
  require_pattern README.md \
    '\[First-read workflow\]\(docs/first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern README.md \
    '\[Known limitations\]\(docs/known-limitations\.md\)' \
    "known limitations link"

  require_pattern docs/README.md \
    '\[First-read workflow\]\(first-read-workflow\.md\)' \
    "first-read workflow link"
  require_pattern docs/README.md \
    '\[Known limitations\]\(known-limitations\.md\)' \
    "known limitations link"

  require_pattern docs/quickstart.md \
    'local-first' \
    "local-first setup framing"
  require_pattern docs/quickstart.md \
    '\[Client workflow\]\(client-workflow\.md#agent-policy-prompt\)' \
    "agent policy prompt link"
  require_pattern docs/quickstart.md \
    'Call agent_route with root, task, and token_budget' \
    "quickstart agent_route policy"
  require_pattern docs/quickstart.md \
    '\[Adoption checklist\]\(adoption-checklist\.md\)' \
    "adoption checklist link"

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
  require_pattern docs/client-workflow.md \
    'Call `agent_route` with `root`, `task`, and `token_budget`' \
    "client workflow agent_route path"
  require_pattern docs/agent-prompt-template.md \
    'call agent_route with root, task, and' \
    "agent prompt agent_route path"
  require_pattern docs/first-read-workflow.md \
    '`agent_route` is the default first-read contract' \
    "first-read workflow agent_route contract"
  require_pattern docs/mcp-client-config.md \
    'Call `agent_route` with `root`, `task`, and `token_budget`' \
    "MCP client config agent_route flow"
  require_pattern docs/adoption-checklist.md \
    'scripts/installed-quickstart-smoke\.sh' \
    "adoption installed quickstart gate"
  require_pattern docs/adoption-checklist.md \
    'CLI `agent-route`, MCP stdio, and MCP `agent_route`' \
    "adoption CLI and MCP agent_route coverage"
  require_pattern docs/adoption-checklist.md \
    'The agent calls `agent_route` with `root`, `task`, and `token_budget` before' \
    "adoption agent_route first-read policy"

  require_pattern docs/maintainer-checklist.md \
    'local-first MCP code context routing' \
    "maintainer focus statement"
  require_pattern docs/maintainer-checklist.md \
    '\[Known limitations\]\(known-limitations\.md\)' \
    "known limitations link"

  echo "docs positioning smoke passed"
}

main "$@"
