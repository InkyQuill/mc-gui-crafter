# Feedback and Session Logs

MCGUI Crafter writes one JSONL session log per app launch:

```text
~/.config/mc-gui-crafter/logs/session-*.jsonl
```

The active log records UI actions, MCP tool calls, export preview warnings and
errors, validation warnings, and AI/user feedback reports.

## What to Attach

For useful bug reports, attach:

- the latest `session-*.jsonl` file
- the `.mcgui` project file when it is safe to share
- the generated export directory or relevant `project_export_preview` output
- `project_render` output or screenshots for visual issues
- exact steps that reproduce the problem

Session logs may include local file paths, project names, element IDs, export
package/class names, and user-written report text. Review logs before sharing if
your paths or project names are sensitive.

## Issue Template

```text
Title:

MCGUI Crafter version:
Operating system:
Loader target:

What happened:

What you expected:

Steps to reproduce:
1.
2.
3.

Attachments:
- session log:
- .mcgui project:
- render/screenshot:
- export output:
```

## AI Agent Workflow

When an AI agent discovers confusing behavior, bad output, missing validation,
or a workflow problem through the MCP server:

1. Call `session_report`.
2. Include a short `summary`, `severity`, and concrete `details`.
3. Tell the user the report was written to the session log.
4. Ask the user to attach the latest `session-*.jsonl` file when filing the
   issue.

Example MCP arguments:

```json
{
  "summary": "Export preview warning does not explain canvas resize options",
  "severity": "warning",
  "details": "Project visible bounds were 232x168 while gui_size stayed 230x168. The user needed guidance to either keep the mismatch intentionally or resize the project."
}
```
