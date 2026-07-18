---
name: skills-manager-orca
description: Integrate Orca runtime health, topic discovery, shared agent skills, and workspace context into Skills Manager. Use when inspecting Orca, adding Orca provider behavior, or deciding whether a capability is read-only versus a local skill binding.
---

# Skills Manager Orca

Keep Orca runtime capabilities separate from local filesystem skill bindings.

## Read-only inspection

- Run `orca status --json` for app and runtime health.
- Run `orca skills list --json` for bundled topic metadata.
- Apply a short timeout and terminate a hung child process.
- Represent missing CLI, failed commands, malformed JSON, offline runtime, and empty topics as explicit unavailable/empty states.
- Sanitize command errors before returning them to the UI or CLI.

## Binding rules

- Do not invent an `.orca/skills` directory or mutate undocumented Orca files.
- A local skill enabled for a consuming tool is not automatically an Orca-native topic.
- Treat `~/.agents/skills` as a shared root and report all configured consumers before changing it.
- Keep Orca topic rows read-only until Orca exposes a stable activation contract.

Verify both running and unavailable Orca states and confirm the provider inventory still lists local direct skills when Orca is offline.
