---
name: skills-manager-tauri
description: Implement and verify Skills Manager Rust/Tauri services, commands, scanners, linkers, Codex adapters, and provider operations. Use when changing backend skill activation, project scope, provider discovery, operation reports, or the inspect CLI.
---

# Skills Manager Tauri

Implement backend behavior in the shared Rust services before editing interface adapters.

## Workflow

1. Read `IMPLEMENTATION_PLAN.md` and inspect the existing service, command, model, and test paths.
2. Put reusable behavior in `src-tauri/src/services/skill_control.rs`, `tool_control.rs`, or the provider-specific service; keep Tauri commands thin.
3. Reuse `ScannerService` for global/project/direct skill discovery and `LinkerService` for link, junction, and copy-mode state.
4. Route Codex plugin state through `codex_config.rs`; preserve unrelated TOML sections and line endings.
5. Gate activation through provider capabilities. Disabled or undetected providers may still expose direct skills, but read-only providers must reject mutation.
6. Return operation reports with applied, skipped, failed, and impacted-provider details. Do not fail fast before recording partial results for batch or preset operations.

## Safety rules

- Never remove a canonical skill from the manager directory during a provider toggle.
- Use `symlink_metadata` when a disabled suffix or broken link must remain observable.
- Keep inspect/preview commands read-only and make them safe for a missing CLI or malformed JSON.
- Preserve existing command payload compatibility unless a new response field is additive.

## Verification

Run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml --bins`, and `cargo test --manifest-path src-tauri/Cargo.toml`. Exercise `skills-manager-inspect providers`, `bindings`, and `skill preview` against a read-only configuration.
