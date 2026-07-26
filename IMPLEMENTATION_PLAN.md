# Skills Manager — Skill Control Plane Implementation Plan

> Status: provider inventory, capability-gated activation, operation reporting, local skill packages, and Vercel Skills workspace/repository registration complete
> Last audited: 2026-07-26 (Asia/Seoul)
> Scope: unified skill inventory, provider-aware activation/deactivation, and Orca integration

## Current implementation slice

- The global/project scope scanner is the read path for both the UI and the standalone inspector.
- `SkillControlService` and `ToolControlService` are the shared mutation boundary for Tauri commands and `skills-manager-inspect`.
- `ProviderInventoryService` and `OrcaService` expose filesystem/config-file/CLI providers, the shared `~/.agents/skills` directory, and Orca runtime/topics without treating Orca topics as local activatable skills.
- Presets target one selected agent and include manager-owned skills plus direct skills owned by that agent.
- Direct skills from disabled manager tools remain visible and actionable.
- `skills-manager-inspect` reports managed skills, direct Tool skills, tool state, projects, Skill Groups, and presets without opening the UI or writing configuration.
- Workspace/repository registration is shared by the UI and `skills-manager-inspect project ...`; repository roots discover `skills/`, `.agents/skills/`, `.claude/skills/`, `.codex/skills/`, and supported agent-specific skill roots.
- Project scanning preserves the canonical `skills/` priority, deduplicates the same skill id across compatible roots, and retains the repository root so skills added later are discovered.
- Project-scoped activation resolves each managed agent to the registered repository's local config/skills directory; legacy bindings without `root_path` retain their existing global targets.
- Existing legacy project bindings continue to deserialize; new bindings persist `root_path` alongside `skills_dir` for workspace-aware discovery.
- Legacy bindings that only persisted a standard `skills_dir` are upgraded on load by inferring and persisting the repository root; custom non-standard paths remain unchanged for safety.
- `skills-manager-inspect providers --json` reports provider capabilities, detection/reachability, skill counts, shared-directory warnings, and Orca status/topics without writing configuration.
- `list_skill_bindings` and `skills-manager-inspect bindings --json` report per-skill provider state, including direct tool paths, managed link/copy targets, shared-directory conflicts, and unavailable providers without writing configuration.
- Single toggles expose `skill preview`/`preview_skill_operation`; shared-root changes identify affected consumers before UI confirmation, and read-only providers reject mutation through the shared capability boundary.
- Single, batch, and preset activation paths return an auditable `SkillOperationReport` with requested, attempted, applied, skipped, failed, and impacted-provider fields.
- Repo-local role skills live under `skills/skills-manager-{architecture,tauri,ui,orca,testing}/SKILL.md` with validated `agents/openai.yaml` metadata.
- CLI parity covers skill/tool toggles, imports, deletes, batch actions, preset apply/list/clear, and preset create/delete/capture/member editing.
- Orca-native topics remain read-only inventory entries; local skill bindings are reported separately and never inferred from an Orca topic name.
- Final audit completed against the provider matrix: Orca reachable/unavailable handling, shared-root impact/guard behavior, Codex plugin state, duplicate IDs, project scope, and read-only providers are covered by runtime checks and tests.
- Vercel Skills compatibility audit completed with repository preview, project registration, active-project switching, removal, Windows path normalization, empty Git repository registration, project-local activation, and multi-root scanner fixtures.
- Repository ownership is separated through `upstream/main`, `patches/skills-manager-control-plane`, and the integrated `main` branch; see `PATCH_GUIDE.md` and `DEVELOPMENT.md`.
- A regression fixture verifies that a skill added after the initial project scan is discovered on the next explicit project-scope scan.

## Interface and scope map

The UI and `skills-manager-inspect` use the same Rust service boundary. The interface is split by the user's intent rather than by duplicating provider logic:

| Surface | Primary responsibility | Default read scope | Mutation target |
|---|---|---|---|
| Skills | Unified skill artifacts, provider bindings, state/source filters, direct installed skills, and per-binding toggles | Global plus direct tool skills | Selected skill instance + selected provider |
| Tools | Detected providers and skills installed directly in an agent directory; tool-wide bulk toggles | Global plus direct tool skills | Selected provider's skill bindings |
| Presets | Reusable skill sets assigned to one agent/provider and one global or project scope | Selected scope | Managed skills and direct skills for the selected target |
| Inspector CLI | Read-only inventory plus parity commands for skill, tool, batch, preset, and project operations | Global when omitted | Explicit command arguments; shared mutations require confirmation |

### Scope rules

- Global is the default. Omitted project arguments never silently follow the active project.
- A project read or mutation requires an explicit project ID. The UI's project selector supplies that ID to scanner, provider inventory, bindings, previews, and preset operations.
- Tools intentionally stays global/direct because it represents agent directories rather than a repository workspace. Project-local activation belongs in Skills and Presets.
- Tool-scoped skills are retained in every scope view when they are installed in the owning agent directory, including when the manager-level tool is disabled.
- A skill instance is identified by its scope-aware `instance_id`; the legacy artifact ID is not sufficient for project/tool operations.

### State and safety rules

- `Skill` is the canonical artifact; `SkillBinding` is the provider-specific state. Enable/disable never mutates or deletes the canonical source.
- `enabled`, `disabled`, `missing`, `conflict`, and `unavailable` are distinct states. A conflict is surfaced for review rather than silently adopted or overwritten.
- `~/.agents/skills` is a shared provider. The UI previews all configured consumers before a change, and CLI mutations require `--confirm-shared`.
- Orca-native topics are inventory-only and read-only. They are not treated as local installed skills and cannot be toggled through the filesystem activation path.

### CLI quick reference

```bash
# Global inspection (default)
npm run inspect -- inspect --json
npm run inspect -- providers --json
npm run inspect -- bindings --json

# Project inspection
npm run inspect -- inspect --project <project-id> --json
npm run inspect -- providers --project <project-id> --json
npm run inspect -- bindings --project <project-id> --json
```

Use `npm run inspect -- --help` for mutation commands. The CLI and Tauri commands call the same `SkillControlService`, `ProviderInventoryService`, and workspace services; the CLI is not a second implementation of activation behavior.

## 1. Current baseline

### Repository

- Repository: `C:\Users\minec\Skills-Manager`
- Branch: `main`
- The implementation slice is committed on `main`; documentation updates are committed separately from feature changes.
- `DESIGN.md` is a visual/style reference, not an implementation plan. This document is the implementation source of truth for the skill control-plane work.

### Verified build baseline

- `npx tsc --noEmit --pretty false --incremental false`: passed.
- `npx vite build`: passed.
- `cargo check --manifest-path src-tauri\\Cargo.toml --bins`: passed.
- `cargo test --manifest-path src-tauri\\Cargo.toml -- --test-threads=1`: passed, 316 tests.
- `cargo fmt --manifest-path src-tauri\\Cargo.toml -- --check`: passed.
- `npm test`: passed, 281 tests.
- `npm run inspect -- providers --json`: passed for the global scope.
- `npm run inspect -- providers --project <project-id> --json`: passed for an explicit project scope.
- Five repo-local skill packages passed `quick_validate.py` with generated `agents/openai.yaml` metadata.
- The latest inspector observed the Orca CLI but received a status timeout; the provider reports this as unavailable rather than as an empty local skill inventory.
- The previous Vite font-reference, large-chunk, stale Browserslist, and Rust `editor_detector` warnings were fixed. A small set of non-blocking Rust visibility/dead-code warnings remains in the upstream-integrated code.
- `caniuse-lite` and `baseline-browser-mapping` were refreshed in `package-lock.json`; the generated route chunks stay below the Vite 500 kB warning threshold.

### Existing application capabilities

The current code already provides most of the first-generation control surface:

- `SkillScope`: global, project, and tool scopes.
- Skill scanning, metadata, package/group membership, marketplace sources, and project bindings.
- Per-tool skill enable/disable through `scanner.rs`, `linker.rs`, and `commands/skills.rs`.
- Tool-level and skill-level bulk actions.
- Presets for capturing and applying activation states.
- Codex plugin `config.toml` enable/disable handling through the provider adapter (`src-tauri/src/services/codex_config.rs`).
- React pages for unified skills and tool-oriented management: `src/pages/Skills.tsx` and `src/pages/Tools.tsx`.

The existing `Tool` model remains compatible as the agent detector and activation target, while `SkillProvider`/`SkillBinding` provide the richer control-plane model required for shared roots, config-file state, and read-only Orca inventory.

## 2. Audited local runtime configuration

| Area | Current state | Meaning for the product |
|---|---|---|
| PowerShell | PowerShell 7.6.3 is active; Windows PowerShell 5.1 is also installed | CLI adapters must work from both shells on Windows |
| Python | Anaconda `conda 25.7.0` is available | Do not model Orca as a Python/conda package |
| Orca app | `orca status --json` returned `ok: true`, runtime `ready` and reachable | Orca health can be surfaced as an integration status |
| Shared skill root | `C:\Users\minec\\.agents\\skills` | Contains `computer-use`, `find-skills`, `orca-cli`, and `orchestration` |
| Codex skill root | `C:\Users\minec\\.codex\\skills` | Contains built-in/system skills and links; it is not the same root as `.agents\\skills` |
| Orca data | `C:\Users\minec\\.orca\\agent-hooks` | Runtime hooks only; no `.orca\\skills` directory was found |
| Orca bundled topics | `computer-use`, `linear-tickets`, `orca-cli`, `orca-emulator`, `orca-emulator-android`, `orca-linear`, `orca-per-workspace-env`, `orchestration` | Distinguish “available in Orca” from “installed in a local agent skill directory” |

The current local installation therefore has two different concepts that must not be conflated:

1. Skills installed into an agent-readable directory, such as `~/.agents/skills`.
2. Orca's own runtime/CLI capabilities, queried through `orca status` and `orca skills`.

Orca's current documentation says the CLI ships with the desktop app and is registered from `Settings → Experimental → CLI`. Its skill registry documents `orca-cli`, `orchestration`, `computer-use`, `orca-linear`, and `orca-emulator`; the CLI can list and retrieve bundled guides with `orca skills list/get`. See:

- https://www.onorca.dev/docs/cli/overview
- https://www.onorca.dev/docs/cli/skills

## 3. Product goal

Provide one UI that can answer:

- Which skills exist locally, in a project, in a marketplace source, or in a shared registry?
- Which agent/provider can currently see each skill?
- Is the skill enabled, disabled, missing, conflicting, stale, or unavailable because its provider is offline?
- What will change if the user toggles it?
- Is Orca healthy and which Orca-native skill guides/capabilities are available?

Activation must be explicit about its target. “Disable Orca” must not silently mean “delete or rename a skill in `.orca`”, because `.orca` is not the observed skill root.

## 4. Target domain model

Keep the existing `Tool`/`Skill` API compatible during migration, then introduce provider-aware concepts alongside it.

### 4.1 Skill artifact

Represents the canonical skill or package member.

```text
SkillArtifact
- artifact_id / stable skill id
- package_id and member_id, optional
- name, description, version
- source: local | imported | marketplace | vault | registry
- canonical_path, optional
```

### 4.2 Skill provider

Represents where a skill is discovered or how it is controlled.

```text
SkillProvider
- provider_id: codex | claude-code | agents-directory | orca | custom
- kind: filesystem | config_file | cli | marketplace
- display_name
- root_path, optional
- detected
- cli_available / reachable
- capabilities: list, install, enable, disable, update, inspect
```

### 4.3 Skill binding

Represents an artifact's state for a provider and scope.

```text
SkillBinding
- artifact_id
- provider_id
- scope: global | project | tool
- state: enabled | disabled | missing | conflict | unavailable
- source_path, optional
- target_path, optional
- last_checked_at
- reason, optional
```

This separates a skill's identity from the mechanism used to activate it. It also avoids using a single `HashMap<String, bool>` as the only representation of state when a provider can return richer statuses.

## 5. Provider strategy

### 5.1 Filesystem providers

Reuse the current scanner/linker behavior for Claude Code, Codex, and other filesystem-backed tools.

- Preserve instance IDs and scope semantics.
- Keep canonical skill files immutable during enable/disable operations.
- Make link/copy/rename behavior an explicit provider capability.
- Continue to support Windows symlink restrictions and copy-mode providers.

### 5.2 Codex provider

Codex needs two state channels:

1. Skill files under its configured skill directory.
2. Plugin state in `config.toml`.

The Codex provider adapter in `codex_config.rs` owns plugin state rather than leaving it embedded as a special case in the generic scanner. Preserve its section matching, line endings, comments, and unrelated TOML sections.

### 5.3 Shared `agents-directory` provider

Add a first-class provider for `~/.agents/skills` because the current global `npx skills add` flow installs the Orca skills there.

- Detect the directory independently of Vercel Skills branding.
- Show it as a shared registry/provider with an impact warning.
- Track which agents consume the directory before changing a skill.
- Avoid duplicate rows when the same skill is visible through a shared root and a tool-specific link.

### 5.4 Orca provider

Do not add Orca as a normal `.orca\\skills` filesystem tool. Add an Orca integration/provider with CLI capabilities:

- `orca status --json`: health and reachability.
- `orca skills list --json`: available bundled topics.
- `orca skills get <name> --json` or `--full`: retrieve the guide/metadata for a topic.
- Optional later adapters for worktrees, terminals, browser automation, orchestration, and emulator capabilities.

For v1, Orca activation should mean enabling the underlying agent skill or shared registry binding. Orca-native topic availability should be displayed separately from filesystem activation. Do not expose an enable/disable switch for an Orca-native topic until a stable command or documented storage contract exists.

### 5.5 Marketplace and registry providers

Keep marketplace install/update separate from activation. A remote skill can be:

- available remotely,
- installed locally,
- enabled for one or more providers,
- or installed but inactive.

The UI should not represent “installed” as “enabled”.

## 6. Implementation phases

### Phase 0 — Baseline protection

- Preserve the existing global/project/tool behavior while extending it through the provider boundary.
- Add this plan and a provider/Orca integration design note.
- Capture fixture snapshots for `.agents/skills`, `.codex/skills`, `.orca/agent-hooks`, and a Codex `config.toml` (see `src-tauri/tests/fixtures/provider-inventory`).
- Keep build/test commands in the handoff checklist.

### Phase 1 — Provider inventory API

Current status: implemented for the local filesystem/config-file providers and the Orca CLI provider; `list_skill_bindings` is exposed to both Tauri and the standalone inspector.

Add a read-only backend command, for example `list_skill_providers`, returning:

- provider identity and kind,
- paths and detection state,
- CLI availability/reachability,
- capability flags,
- skill counts and warning messages.

Add a separate `list_skill_bindings` or extend `list_skills` with provider binding records. Avoid breaking the current frontend response until the new UI consumes the new shape.

### Phase 2 — Shared skill and Orca adapters

Current status: read-only `agents-directory` scanning and Orca status/topic parsing are implemented with timeout and malformed-response handling. Shared activation is routed through consuming filesystem providers; Orca topics remain read-only.

- Implement the `agents-directory` scanner.
- Implement an Orca CLI service with timeouts, JSON parsing, unavailable/offline states, and sanitized error messages.
- Add fixtures for successful Orca responses, empty topic lists, malformed JSON, missing CLI, and offline app.
- Add the installable Orca topic catalog as metadata, not as hard-coded active files.

### Phase 3 — Activation semantics

Current status: capability checks, shared-root previews, and operation reports are implemented across single toggles, batches, and presets. Preset UI consumes failed-operation reports; shared-root confirmation is enforced for direct UI toggles and CLI mutations require `--confirm-shared`.

- Route existing filesystem toggles through provider capabilities.
- Route Codex plugin toggles through the Codex adapter.
- Define shared-root behavior: preview impacted agents, confirm broad changes, and never delete canonical source files.
- Return an operation report with applied, skipped, and failed bindings.
- Keep the current batch/preset behavior by translating presets into binding operations.

### Phase 4 — UI inventory and control surface

Current status: provider inventory, provider/state/source filters, and expanded skill binding rows are visible on the Skills page. Provider filtering keeps disabled bindings visible and separates Orca topics from local skill artifacts.

Refactor the existing Skills page incrementally:

- Provider filter: Codex, Claude Code, shared agents directory, Orca, project, custom.
- State filter: enabled, disabled, missing, conflict, unavailable.
- Source filter: local, marketplace, registry, vault.
- Separate cards/rows for skill artifacts and provider bindings.
- Show the activation target and impact before a toggle.
- Orca integration card: app running, CLI available, runtime reachable, topic count, last checked time.
- Use disabled controls with reasons when a provider is read-only or unavailable.
- Preserve current batch actions and presets through the new binding model.

### Phase 5 — Local skill package organization

Current status: five concise repo-local role skills and generated interface metadata are present and validated.

Add repo-local skills using the standard top-level layout:

```text
skills/
├─ skills-manager-architecture/SKILL.md
├─ skills-manager-tauri/SKILL.md
├─ skills-manager-ui/SKILL.md
├─ skills-manager-orca/SKILL.md
└─ skills-manager-testing/SKILL.md
```

The Orca documentation confirms that repositories with `skills/<name>/SKILL.md` can be installed through `npx skills add`. Each local skill should contain only the workflow needed for that role and link back to the provider model in this plan.

### Phase 6 — Verification and release readiness

Current status: complete. Backend, frontend, CLI, local skill metadata, and the provider matrix are green. The implementation worktree is clean after topic-based commits; operational conflicts are reported as state and are not auto-overwritten.

- Rust unit tests for each provider adapter and state transition.
- TypeScript tests for grouping, filtering, status labels, batch operations, and optimistic rollback.
- Integration fixtures for Windows path normalization and symlink/copy behavior.
- Provider matrix verification completed with:
  - Orca CLI detection plus timeout, missing/offline/malformed response tests;
  - shared `~/.agents/skills` installed, including read-only preview and confirmation guard;
  - Codex plugin enabled/disabled state tests;
  - duplicate skill IDs across provider/scope fixtures;
  - project-scoped scanner and activation tests, including discovery after a skill is added;
  - a provider without enable/disable capability rejected at the shared mutation boundary.
- Final handoff commands passed: `npm test`, `npm run build`, `cargo fmt --manifest-path src-tauri\\Cargo.toml -- --check`, `cargo check --manifest-path src-tauri\\Cargo.toml --bins`, `cargo test --manifest-path src-tauri\\Cargo.toml -- --test-threads=1`, inspector global/project smoke tests, and `git diff --check` for source/document changes.

## 7. Acceptance criteria

- The UI lists skill artifacts without duplicate rows caused by shared roots or linked targets.
- Every activation toggle names its provider and scope.
- Enabling/disabling a skill updates only the selected binding and does not delete the canonical artifact.
- Codex plugin state and filesystem/link state remain consistent after refresh.
- Shared `~/.agents/skills` changes display their cross-agent impact before confirmation.
- Orca health and available topics are visible even when no Orca-owned skill directory exists.
- Orca offline/missing CLI is represented as `unavailable`, not as an empty skill inventory.
- Batch operations and presets produce an auditable operation report.
- Existing global/project/tool behavior remains backward compatible.
- UI and CLI expose the same provider inventory, scope rules, shared-root confirmation, and operation-report semantics.

## 8. Out of scope for v1

- Executing arbitrary shell commands from the UI.
- Mutating undocumented Orca internal files.
- Treating Orca's bundled topic list as a local install manifest.
- Cloud synchronization of provider credentials or CLI sessions.
- Automatic installation of all available Orca topics.
- Replacing the current `Tool` model in one large migration.

## 9. Original recommended first implementation slice (completed)

Implement the smallest vertical slice first:

1. Add `SkillProvider`/capability types without removing `Tool`.
2. Add `agents-directory` inventory for `~/.agents/skills`.
3. Add read-only Orca status/topic inventory.
4. Render both in the existing Skills page behind a provider filter.
5. Add tests and only then route enable/disable through the new adapter boundary.

This gives the user immediate visibility into the current Orca/shared-skill setup while reducing the risk of changing the already-working filesystem/link behavior.
