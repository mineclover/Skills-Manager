---
name: skills-manager-ui
description: Build and review the Skills Manager React UI for provider-aware skill inventory, binding states, presets, filters, scope selection, and operation feedback. Use when changing Skills or Presets while preserving CLI parity.
---

# Skills Manager UI

Keep the UI a presentation and interaction layer over the shared Tauri services.

## Workflow

1. Read the relevant Rust command signature and `src/types/index.ts` before adding an invoke call.
2. Keep provider identity, scope, and binding state visible near every activation control.
3. Load global state by default; pass an explicit `projectId` when the user selected a project. Refresh skills, bindings, and provider inventory together after mutations.
4. Keep direct skills from disabled tools visible and actionable. Do not hide a row because `enabled[provider]` is false.
5. Show `enabled`, `disabled`, `missing`, `conflict`, and `unavailable` distinctly. Explain read-only or shared-root restrictions instead of silently disabling controls.
6. Use the operation preview before a shared-root toggle and show operation-report failures after batch or preset actions.

## Parity checklist

- Provider filters use the same provider IDs as the CLI.
- Project/global selectors map to the same scanner scope as `skills-manager-inspect`.
- Presets target one selected agent and include manager-owned plus direct skills for that agent.
- UI refreshes from disk after every activation, import, delete, or preset application.
- Add focused pure-function tests for grouping, filtering, state labels, and selection behavior.

Run `npm run build`, `npm test -- --run`, and the focused skill-list tests before handoff.
