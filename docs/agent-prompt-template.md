# Agent Prompt Templates

Use these templates in Codex, Claude Code, Cursor, or another MCP-capable
coding agent when CodeInsight is configured for the current repository.

The templates are intentionally direct. They tell the agent when to call
CodeInsight, when to read files, when to continue, and when to switch from
navigation into edit planning.

## Base Repository Policy

Use this as a persistent instruction when CodeInsight MCP is available:

```text
When CodeInsight MCP is available for a repository:

1. Before broad code reading, call agent_route with root, task, and
   token_budget for the default first read.
2. Read context_pack.files in reading_plan order. Treat reading_plan.reason as
   the current-step instruction, reading_plan.question as the local reading
   checklist, and reading_plan.selection_reason as the selection evidence.
3. Prefer reading_plan[].suggested_tool for deeper evidence on the current
   selected file.
4. Use continuation_summary.suggested_tool only after the selected context has
   been consumed and the task still needs more evidence.
5. Before editing, review the included impact_analysis preview. If the edit
   target differs from the first-read seed, call impact_analysis with the
   selected files or symbols and run or report the suggested_checks that apply.
6. Use index_project, project_overview, context_pack, and impact_analysis
   directly only when custom routing or partial refresh control is needed.
7. Treat CodeInsight call graphs and references as best-effort navigation
   evidence, not compiler-grade proof.
```

## First Read A Repository

Use this when the user asks to understand an unfamiliar repository:

```text
Understand this repository with CodeInsight first.

Workflow:
1. Call agent_route with:
   - root: repository root
   - task: "understand the repository structure, primary entrypoints, and main
     execution flow"
   - token_budget: 6000
2. Read selected files in reading_plan order.
3. Use each reading_plan.reason to decide what the current file should answer;
   use selection_reason only to explain why the file was selected.
4. Summarize:
   - primary purpose
   - likely entrypoints
   - important directories
   - files already inspected
   - remaining unknowns
   - recommended next CodeInsight tool calls

Do not scan the whole repository unless the selected context is insufficient.
```

## Change Preflight

Use this before making a code change:

```text
Before editing, use CodeInsight to estimate the local change radius.

Workflow:
1. If the task starts from a broad repository question, call agent_route first.
2. Identify the target file, symbol, or module from the user's request.
3. Call context_pack for the target if more local context is needed.
4. Call impact_analysis with the target files or symbols.
5. Review:
   - risk_level
   - impact_counts
   - impacted_files
   - paths
   - suggested_checks
6. Make the smallest edit that satisfies the task.
7. Run the applicable suggested_checks, or explain why a check could not run.

Do not treat a low impact score as proof of safety. Use it as routing evidence.
```

## Continue After Budget Limits

Use this when the first context pack does not fully answer the task:

```text
Continue with CodeInsight only after reading the selected context.

Workflow:
1. Check context_pack.continuation_summary.status.
2. If status is "complete", do not fetch more context unless the selected files
   fail to answer the user's task.
3. If continuation_summary.suggested_tool exists, execute that suggested tool.
4. Prefer omitted_candidates that match the current unresolved question.
5. Keep the follow-up task narrower than the first task.
6. Report what changed after the continuation:
   - new files read
   - new symbols or references found
   - remaining uncertainty
   - whether another continuation is justified
```

## Review Planning

Use this when preparing a review, PR risk summary, or refactor plan:

```text
Use CodeInsight to turn review planning into a bounded evidence pass.

Workflow:
1. Call agent_route with the review task and a bounded token_budget.
2. Read selected files in reading_plan order.
3. Call impact_analysis for each changed or planned target.
5. Use callers, callees, find_references, or dependency_graph only when the
   review question requires flow or dependency evidence.
6. Report:
   - expected behavior surface
   - risk level and impacted files
   - tests or checks to run
   - areas intentionally not verified

Keep findings grounded in files and tool output. Do not claim compiler-grade
certainty from CodeInsight navigation evidence.
```

## Minimal One-Shot Prompt

Use this compact prompt when the agent only supports a short custom
instruction:

```text
Use CodeInsight before broad repository reading: call agent_route with
root/task/token_budget. Read selected files in reading_plan order. Use
continuation only after selected context is consumed. Review impact_analysis
before edits. Treat call graphs and references as best-effort navigation
evidence, not compiler-grade proof.
```

## Recommended Defaults

- First repository read: `token_budget` 6000.
- Focused file or module task: `token_budget` 3000-5000.
- Large refactor planning: start at `token_budget` 8000, then use continuation
  instead of one very large pack.
- Always prefer `context_pack` before raw broad file reads unless the user
  names an exact file and line.

## Related Docs

- [Client workflow](client-workflow.md)
- [MCP client configuration](mcp-client-config.md)
- [First-read workflow](first-read-workflow.md)
- [Recommendation contract](recommendation-contract.md)
- [Known limitations](known-limitations.md)
