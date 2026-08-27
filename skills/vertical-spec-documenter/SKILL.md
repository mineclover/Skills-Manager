---
name: vertical-spec-documenter
description: Build, compile, and validate bounded Vertical Topic Specification documents for autonomous and HT maintenance cycles. Treats horizontal and vertical contexts as relative fractal planes with canonical topic IDs, owned scopes, strict invariants, and pinpoint verification gates.
---

# Vertical Spec Documenter (Sujik Spec Documentation)

Authorized context compiler for pin-point problem resolution and inner-loop TDD cycles.

## 1. Relative Horizontal / Vertical Fractal Principle

Horizontal and Vertical contexts are **not fixed absolute layers**. They are **relative to the agent's local reference plane**:

- **Parent Plane (Level 0)**: The topic (`topic:auth_subsystem`) is a specific *Vertical Topic* selected from a broad Horizontal system survey.
- **Local Plane (Level 1)**: Upon drilling down, that same topic becomes the agent's *Local Horizontal Plane*. Inside it, new finer-grained Vertical Sub-Topics (`topic:auth_subsystem/jwt_latency`) are spawned.
- **Roll-Up(**: Upon resolution, the verified evidence and ContextPatchProposal are rolled up into the parent plane's baseline.

## 2. Mandatory Specification Sections

All Vertical Spec documents MUST contain the following 5 structured sections:

### 1. Topic Identity & Lineage
- `Topic ID`: Canonical unique identifier (e.g. `topic:domain/problem_id`).
- `Lineage Path`: Full hierarchical path from root (e.g. `root -> topic:domain -> topic:domain/subtopic`).
- `Lifecycle State`: `IN_PROGRESS`, `VERIFIED`, `REOPENED`, `CLOSED`.

### 2. Local Horizontal Scope
- `Owned Target Files`: Exact files that this vertical topic is authorized to modify.
- `Read-Only Interfaces`: immutable types, schemas, or contracts used for reference only.
- `Prohibited Out-of-Bounds`: Areas strictly forbidden from mutation to prevent regressions.

### 3. Concrete Behavioral Invariants
- `Pre-conditions`: State required prior to execution.
- `Post-conditions`: Deterministic expected outcomes after execution.
- `Strict Invariants`: Non-negotiable rules (type safety, memory, latency ceilings, token budgets).

### 4. Targeted Verification Mechanism
- `Target Test File`: EXACTLY one scoped test file (e.g. `apps/foo/test/bar.test.js`).
- `Allowed Command`: Pinpoint test command (e.g. node --test apps/foo/test/bar.test.js).
- `Prohibited Commands`: `npm test`, `pytest`, and global regression sweeps (Test Storm Suppression) are STRICTLY BLOCKED during this vertical topic's inner loop.

### 5. Acceptance & Roll-Up Gate
- Clear checklist that must pass before the solution is promoted to the parent horizontal plane.

---

## 3. Selective Information Condensation Guide (80k Density Rubric)

The "80k Token Budget" is not merely an arbitrary text limit; it is the **Optimal High-Signal Information Density Ceiling** ensuring the agent maintains 100% reasoning fidelity without context pollution or attention dilution.

### 3-Tier Condensation Matrix:

| Tier | Strategy | Content Items |
|---|---|---|
| **Tier 1: 100% Raw Preservation** | **Exact Fidelity** | • Canonical `Topic ID` & Lineage Path<br>• Public AST / Type / Interface Signatures<br>• Pre/Post Conditions & Strict Invariants<br>• Pinned Target Test File (`1:1` Binding)<br>• Active Normative Decisions (ADRs) |
| **Tier 2: Structural Distillation** | **Diff & Summary** | • Source Code $\rightarrow$ **AST Signatures & Diff Patches**<br>• Multi-turn Trial & Error $\rightarrow$ **Single Decision Summary**<br>• Verbose Test Logs $\rightarrow$ **Pass/Fail Evidence Matrix** |
| **Tier 3: Zero-Noise Pruning** | **Complete Discard** | • Ephemeral debugging thoughts & chat conversational fluff<br>• Out-of-bounds file implementation details<br>• Intermediate broken stack traces already resolved<br>• Raw package manager lockfiles / dependency tree dumps |

