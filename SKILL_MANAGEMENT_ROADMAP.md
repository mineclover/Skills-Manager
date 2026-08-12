# Skills Manager — Skill Set Studio & Activation Control Roadmap

> Status: proposal
>
> Scope: skill-set definition, project/work-scope assignment, provider-aware activation,
> feedback, evaluation, and health management.
>
> Relationship: this document extends the implemented provider-aware control plane in
> [`IMPLEMENTATION_PLAN.md`](./IMPLEMENTATION_PLAN.md). It does not replace its
> provider, scope, or filesystem safety rules.

## 1. Product direction

Skills Manager is a **skill management system**, not merely a switchboard for agent
tools. It must let users:

1. Define and document a reusable set of skills for an intended kind of work.
2. Attach that set to a project as a default or recommendation.
3. Add a work-specific set for the task currently being performed.
4. Preview and safely apply the resolved configuration to one or more providers.
5. Capture evidence-backed feedback and use it to improve the skill or skill set.

The primary user question is:

> For this project and this task, which skill configuration should I use, and why?

Enable/disable is the final deployment operation, not the primary product concept.

## 2. Two connected product areas

```text
Skill Set Studio                         Activation Control
----------------                         ------------------
skills, sets, guides, contracts          project, work scope, provider targets
evaluation cases and release versions    preview, apply, drift, operation history
             │                                      │
             └────────── feedback loop ────────────┘
```

### 2.1 Skill Set Studio

Studio owns the definition and quality of a skill set.

- Skill library, source, version, requirements, and documentation.
- Skill-set purpose, membership, work-scope tags, and usage guide.
- Success contract, safety boundaries, evaluation cases, and review cadence.
- Immutable releases that can be assigned to projects.
- Review queue for unhealthy, stale, unsafe, or incomplete assets.

### 2.2 Activation Control

Activation Control owns the safe application of a selected release.

- Project baseline and recommended skill-set assignments.
- Selection of the current work scope and temporary overlays.
- Explicit provider targets and project scope.
- Preview, shared-root confirmation, apply, operation report, and drift view.
- Usage result and feedback capture in the context in which it occurred.

The two areas may initially be pages in one desktop application. They must not
duplicate backend activation behavior or have separate copies of provider state.

## 3. Domain model

### 3.1 Existing canonical records

The following records remain authoritative and are not replaced:

- `Skill`: canonical artifact discovered from a global, project, or direct-tool root.
- `SkillBinding`: provider-specific state of that artifact.
- `SkillOperationReport`: auditable result of an attempted mutation.
- `ProjectBinding`: explicit registered project and its skill roots.

The same artifact may have multiple scope-aware `instance_id` values. New models
must never resolve a project or tool skill by legacy artifact ID alone.

### 3.2 New records

```text
SkillContract
- purpose: summary, use_when, avoid_when
- requirements: providers, runtimes, libraries, project signals, verification commands
- success_contract: expected outcomes, non-goals, safety rules
- feedback_schema: allowed codes and required evidence
- evaluation: cases, review cycle, health thresholds

SkillSetBlueprint
- id, name, description, work_scope_tags
- entries: artifact_id + scope policy + required flag + provider constraints
- guide, contract references, lifecycle state

SkillSetRelease
- blueprint_id, version, released_at, immutable resolved manifest
- release notes and evaluation summary

ProjectSkillSetAssignment
- project_id, skill_set_release_id
- role: default | recommended
- priority, default_provider_ids

ActivationPlan
- project_id, selected releases, work-scope tags, provider_ids
- resolved skill instance IDs, previewed operations, warnings

FeedbackEvent
- release_id, skill instance ID, project ID, provider ID, work-scope tags
- outcome code, evidence, evaluator type, timestamp
```

### 3.3 Scope policy for an entry

A skill-set entry identifies a stable artifact and declares how it should resolve
in a project. Resolution occurs only when an activation plan is built.

| Policy | Meaning |
| --- | --- |
| `global` | Require the global instance. |
| `project` | Require the explicit project instance. |
| `project_then_global` | Prefer the registered project instance, then use global. |
| `tool_local` | Use only a direct skill owned by the selected provider. |

An unresolved required entry is a blocking preview result, not a silent omission.

## 4. Managed skill and skill-set conventions

### 4.1 Contract eligibility

An imported external skill may remain visible and usable without a contract, but it
is labelled **unmanaged**. A skill or skill set may be marked **managed** or
**verified** only when its required contract fields are present.

This preserves third-party compatibility while making quality conventions mandatory
for assets the manager promotes or owns.

### 4.2 Portable sidecars

Keep `SKILL.md` compatible with agent tooling. Store portable management metadata in
a sidecar rather than requiring every consumer to understand additional frontmatter.

```text
my-skill/
├─ SKILL.md
├─ skill-manager.yaml
├─ evaluations/
│  ├─ happy-path.md
│  ├─ edge-cases.md
│  └─ safety-cases.md
└─ references/

skill-sets/upstream-integration/
├─ SKILL_SET.md
├─ skill-set-manager.yaml
└─ evaluations/
```

For assets that cannot be changed on disk, Skills Manager stores equivalent local
metadata keyed by source path and scope-aware instance ID. Local metadata never
overwrites a third-party skill file.

### 4.3 Required contract fields

Every managed asset must declare:

1. Purpose and intended work scope.
2. When to use it and when not to use it.
3. Environment requirements and verification commands.
4. Expected outcomes, non-goals, and safety boundaries.
5. Evaluation cases and review cadence.
6. Feedback codes and the evidence needed for a positive outcome.

Example:

```yaml
schema_version: 1
purpose:
  summary: "Integrate upstream changes while preserving provider-aware controls."
  use_when: ["upstream merge", "integration release"]
  avoid_when: ["a one-file local change"]
requirements:
  runtimes: [node, rust]
  project_signals: [package.json, src-tauri/Cargo.toml]
  verification: ["npm test", "npm run build", "cargo test"]
success_contract:
  expected_outcomes:
    - "The integration records conflict-resolution rationale."
    - "Provider/scope regression checks pass."
  non_goals: ["Silently replacing local control-plane behavior."]
  safety_rules: ["Preview shared-root impact before mutation."]
feedback:
  codes: [completed, partial, failed, instruction_gap, dependency_gap, safety_concern]
  required_for_completed: [verification_evidence]
evaluation:
  cases: [evaluations/upstream-integration.md]
  review_cycle_days: 90
  health_thresholds:
    minimum_verified_success_rate: 0.80
    maximum_correction_rate: 0.20
```

## 5. Feedback and evaluation

### 5.1 Feedback is evidence-backed

A thumbs-up/down is insufficient. Feedback must be attached to the project,
work-scope, provider, and release that produced it.

| Code | Meaning |
| --- | --- |
| `completed` | The declared outcome was achieved with evidence. |
| `partial` | Useful result, but a material correction was needed. |
| `failed` | The intended outcome was not achieved. |
| `wrong_scope` | The skill or set was not suitable for the task. |
| `instruction_gap` | The guide or procedure was ambiguous or incomplete. |
| `dependency_gap` | Required runtime, library, or project context was missing. |
| `safety_concern` | The output needs review before further recommendation. |

An agent's assertion is not success evidence. Verified commands, automated evaluation
assertions, or explicit human confirmation are evidence. Feedback storage must retain
the evidence type and redact secrets, file contents, and personally sensitive data.

### 5.2 Health metrics

Metrics are computed for a skill, a set release, and the combination of project,
work scope, and provider.

| Metric | Definition |
| --- | --- |
| Usage count | Number of observed uses or activation runs. |
| Verified success rate | Evidence-backed `completed` outcomes / evaluated outcomes. |
| Correction rate | `partial` outcomes / evaluated outcomes. |
| Scope mismatch rate | `wrong_scope` outcomes / evaluated outcomes. |
| Failure distribution | Count grouped by feedback code and provider. |
| Safety incidents | `safety_concern` count; never hidden by aggregate health. |
| Freshness | Time since the most recent successful evaluation. |
| Drift rate | Actual binding state differs from the release's intended state. |

Health is `unknown` until there is enough evidence. It must not be shown as healthy
simply because no feedback exists.

### 5.3 Lifecycle

```text
Draft → Reviewed → Verified → Monitored → Needs review → Deprecated
```

- `Draft`: authoring is in progress; no deployment recommendation.
- `Reviewed`: contract is complete and has been reviewed.
- `Verified`: required evaluation cases passed.
- `Monitored`: actively recommended and gathering operational evidence.
- `Needs review`: threshold breach, stale evaluation, or safety issue.
- `Deprecated`: remains inspectable but is not recommended for new assignments.

## 6. Information architecture and workflows

### 6.1 Navigation

```text
Library
├─ Skills
├─ Skill Sets
├─ Releases
└─ Review Queue

Projects
├─ Project Profile
├─ Default Skill Sets
└─ Recommended Work Scopes

Activate
├─ Current project and task
├─ Provider targets
├─ Preview and apply
└─ Drift and history
```

### 6.2 Primary workflow

```text
Choose project
  → load its default releases
  → choose current work scope
  → add recommended release overlays
  → preview resolved provider changes
  → confirm shared-root effects
  → apply and record operation report
  → record feedback after the work is evaluated
```

The first screen should ask for project and work intent, not for individual toggles.
Individual binding controls remain an advanced adjustment surface.

### 6.3 Review queue

The Review Queue must collect:

- releases below their success threshold;
- stale evaluations;
- unresolved required entries;
- repeated dependency or instruction gaps;
- provider-specific drift;
- safety concerns;
- managed assets missing a required contract field.

## 7. Implementation architecture

### 7.1 Reuse the existing control plane

`SkillControlService` remains the only mutation boundary. New set application flow is:

```text
SkillSetService resolves release + project + provider
  → ActivationPlanService creates a preview
  → SkillControlService applies binding operations
  → SkillOperationReport is persisted as activation history
```

`ProviderInventoryService`, `ScannerService`, `LinkerService`, and the Codex adapter
remain authoritative for discovery and actual state. No Studio or UI layer may write
provider roots, symlinks, or `config.toml` directly.

### 7.2 Persistence boundaries

| Data | Storage | Reason |
| --- | --- | --- |
| Skill/Set contract and release manifest | Portable sidecar or manager-owned package | Shareable, versionable, reviewable. |
| Project assignments and UI preferences | Existing app config | Local project intent and defaults. |
| Feedback, evaluations, activation history | Local SQLite | Append-oriented history and metric queries. |
| Provider binding state | Filesystem/config scan | Always reconcile from actual state. |

## 8. Roadmap

### Milestone 0 — Vocabulary and compatibility

- Introduce product names: Skill Set Studio, Activation Control, Blueprint, Release,
  Project Assignment, and Feedback Event.
- Keep existing `Preset` storage and CLI behavior compatible.
- Present existing presets as legacy activation profiles; do not delete or reinterpret
  user data automatically.
- Add this roadmap to repository navigation.

**Done when:** the current feature set remains unchanged and the new terminology is
documented without changing provider behavior.

### Milestone 1 — Skill Contract and managed status

- Add sidecar parsing and local metadata fallback.
- Add managed/unmanaged/verified lifecycle display.
- Validate required contract fields for managed assets.
- Add a contract editor and a read-only summary on the skill detail view.

**Done when:** a managed skill cannot be promoted without purpose, boundaries,
verification, and feedback conventions; imported skills remain usable.

### Milestone 2 — Skill Set Blueprint and Release

- Add provider-neutral skill-set blueprints and scope policies.
- Create immutable releases with membership and contract snapshots.
- Add release notes and evaluation status.
- Allow a set to be built from selected library skills.

**Done when:** a release can explain its purpose, members, required environments,
and evaluation status without selecting a provider.

### Milestone 3 — Project Profiles and work scopes

- Add default and recommended release assignments to registered projects.
- Add work-scope tags and overlay selection.
- Build an effective-set resolver that turns releases into explicit instance IDs.
- Block preview for unresolved required project entries.

**Done when:** a user can open a project, select a work scope, and see the effective
set before mutating any provider.

### Milestone 4 — Activation Control

- Add `preview_skill_set_release` and `apply_skill_set_release` commands.
- Route application through existing `SkillControlService` reports.
- Add provider target selection, shared-root confirmation, drift, and history UI.
- Move bulk provider toggles behind the advanced activation view.

**Done when:** a project + work scope applies a release safely, reports every
operation, and can show why each skill was included.

### Milestone 5 — Feedback, evaluation, and health

- Add structured feedback events with evidence type and redaction.
- Add evaluation-case execution records and threshold calculations.
- Build skill/set/project/provider health views and Review Queue.
- Surface safety concerns separately from aggregate success metrics.

**Done when:** no managed release can be recommended as healthy without evidence,
and threshold breaches reliably create a review item.

### Milestone 6 — Assisted authoring and improvement

- Generate contract and skill-set drafts from selected project context.
- Suggest, but never auto-apply, skill candidates from repeated feedback patterns.
- Detect likely duplicate skills and missing requirements.
- Require human review before a draft is released or assigned by default.

**Done when:** users can turn a repeated project workflow into a reviewed managed
skill or set without turning agent-generated text directly into production policy.

## 9. Acceptance and safety rules

- A user can apply a project work-scope configuration without manually toggling each skill.
- Every mutation names the project, provider, scope, and affected instance IDs.
- Shared roots retain preview and explicit confirmation requirements.
- Direct tool skills, disabled bindings, conflicts, and unavailable providers remain visible.
- Canonical manager skill artifacts are never changed by activation.
- A managed asset always exposes purpose, boundaries, evaluation, and feedback conventions.
- Feedback never treats unverified agent output as a successful outcome.
- All new state transitions have focused Rust fixtures plus frontend and CLI parity tests.

## 10. Explicit non-goals

- Building a general-purpose agent runtime, scheduler, chat service, or sandbox platform.
- Automatically installing arbitrary libraries or executing unreviewed commands from a contract.
- Treating remote marketplace availability as local activation.
- Turning Orca-native read-only topics into filesystem toggles.
- Replacing user skill files or third-party metadata to force a manager convention.

## 11. Implementation readiness and decision gates

### 11.1 Ready now

Milestone 0 can start immediately. It is documentation, navigation, and compatibility
work only: it introduces the product vocabulary while retaining existing presets,
commands, provider behavior, and user data unchanged.

Milestone 1 may start after the decision gates below are recorded in an architecture
decision record and represented by focused fixtures. The gates are intentionally
explicit: they prevent a contract or feedback feature from creating data that cannot
later be interpreted consistently.

### 11.2 Decision gates before persisted Studio data

| Decision | Required resolution |
| --- | --- |
| Stable set membership | A release entry needs an immutable source identity in addition to an artifact ID: source kind/path or package reference, selected scope policy, and the contract revision it was evaluated against. |
| Release immutability | Define whether portable sidecars are copied into the release manifest, content-addressed by digest, or both. A release must remain explainable after its source skill changes. |
| Feedback target | Each event must target exactly one `skill`, `skill_set_release`, or `activation_run`; set-level feedback must not be silently counted as every member skill's result. |
| Evidence and privacy | Define an evidence type (`command_result`, `evaluation_assertion`, `human_confirmation`), retention policy, redaction boundary, and whether raw output is stored at all. |
| Health denominator | Define the minimum evaluated sample size and review window. Until both are met, health stays `unknown`; it cannot be promoted by a high rate from one event. |
| Legacy preset migration | Presets remain activation profiles. New blueprints/releases use new IDs and storage; no automatic conversion or reassignment of user presets is permitted. |

### 11.3 Required first vertical slice

The first implementation slice after Milestone 0 is deliberately read-only:

1. Parse a `skill-manager.yaml` sidecar and validate the managed-skill contract.
2. Display managed status and contract completeness beside an existing skill.
3. Store no feedback, create no releases, and make no activation mutations.
4. Add parser fixtures for valid, incomplete, malformed, third-party, and missing
   sidecars across global/project/tool scope instances.

Only after this slice is green should Studio persistence, releases, assignments, and
Activation Control commands be added.
