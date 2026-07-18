---
name: skills-manager-testing
description: Verify Skills Manager provider bindings, activation reports, project scopes, shared roots, Codex plugin state, and CLI/UI parity. Use when adding or reviewing behavior that can change skill visibility or activation.
---

# Skills Manager Testing

Test the shared service and both interfaces against the same observable state.

## Required matrix

- Managed global skill: enabled, disabled, missing target, wrong target, and broken target.
- Project-scoped skill: default global read versus explicit project read.
- Direct tool skill: enabled, `.disabled-by-sm`, disabled tool, and Codex plugin state.
- Shared `~/.agents/skills`: one consumer, multiple consumers, conflicting consumer states, and read-only inventory.
- Orca: healthy, offline, missing CLI, malformed response, and empty topic list.
- Operations: single toggle, batch, preset target, preset scope, skipped no-op, partial failure, and operation report.

## Commands

Run Rust unit tests, frontend tests/build, then read-only checks such as:

```text
skills-manager-inspect providers --json
skills-manager-inspect bindings --json
skills-manager-inspect skill preview --id <instance-id> --tool <provider-id> --enable --json
```

Assert JSON fields and actual filesystem/config state; a green build alone is not evidence that provider discovery or scope behavior is correct.
