# Public Adoption Feedback Template

Use this template when trying CodeInsight on a real repository. Attach the
generated evidence folder when possible.

## Environment

- CodeInsight version:
- Install method: release installer / Homebrew / source / Docker
- MCP client: Codex / Claude Code / Cursor / other
- Operating system:
- Repository type and language:
- Repository URL or private project description:

## Task

```text
<the exact task passed to agent_route>
```

## Expected First Read

- Expected first file or area:
- Why this would be a good starting point:

## CodeInsight Result

- First selected file:
- First reading focus:
- First reading question:
- First suggested tool:
- Blind baseline source lines:
- Routed first-read source lines:
- Source lines avoided:
- Line reduction:
- Read-less ratio:
- Impact risk:
- Suggested checks:

## Outcome

Choose one:

- `route_hit`: first selected file was useful.
- `route_near_miss`: first selected file was close but not ideal.
- `route_miss`: first selected file was wrong.
- `workflow_friction`: setup, MCP config, prompt, or output shape blocked the trial.
- `overtrust_risk`: output looked stronger than best-effort navigation evidence.

What happened:

```text
<short explanation>
```

## Reproduction

Preferred command:

```bash
scripts/adoption-evidence.sh /path/to/repo \
  --task "<task>" \
  --output-dir /tmp/codeinsight-adoption-evidence \
  --print-snippet \
  --issue-template
```

Attach or link:

- `adoption-evidence.md`
- `summary.json`
- `agent-route.json`
- `mcp-first-call.json`, when available

## Notes

- Did the agent read selected files in `reading_plan[]` order?
- Did it wait to call the suggested tool until after selected context was read?
- Did it review `impact_analysis` before edits?
- Did any output imply compiler-grade certainty or safety proof?
