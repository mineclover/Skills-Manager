---
name: skills-manager-architecture
description: Design and review Skills Manager provider inventory, skill bindings, scopes, presets, shared roots, and CLI/UI parity. Use when changing the control-plane model or evaluating whether a feature preserves canonical artifacts and provider isolation.
---

# Skills Manager Architecture

Use the provider-aware model in `IMPLEMENTATION_PLAN.md` as the design authority.

## Invariants

- Keep `Skill` as the canonical artifact record and `SkillBinding` as the per-provider state record.
- Preserve `global`, `project`, and `tool` instance IDs. Do not resolve a project or tool skill by the legacy artifact ID alone.
- Treat a provider root, not a tool label, as the source of truth for filesystem visibility.
- Never delete or mutate a canonical manager skill while enabling or disabling a binding.
- Treat `~/.agents/skills` as a shared provider. Any operation there must identify all configured consumers.
- Treat Orca topics as read-only runtime capabilities, not local installed skills.

## Design workflow

1. Read the relevant scanner, linker, config, command, and UI code before proposing a new abstraction.
2. Add shared Rust service behavior first, then make Tauri and CLI adapters call that service.
3. Represent unknown, missing, conflicting, and unavailable states explicitly; do not collapse them into `false`.
4. Keep global scope as the default read scope. Require an explicit project ID for project-scoped reads and writes.
5. Return an auditable operation report for mutations, including skipped and failed bindings and shared-root impacts.
6. Add a focused fixture/test for every new state transition and verify both interfaces.

## Review checklist

- Does the change duplicate an artifact because a shared path is visible through multiple providers?
- Can a disabled direct skill still be listed and toggled?
- Does the selected provider and scope appear in every activation operation?
- Does a provider capability prevent unsupported mutation before filesystem changes occur?
- Does a refresh reconcile the on-disk state instead of trusting cached enabled maps?
