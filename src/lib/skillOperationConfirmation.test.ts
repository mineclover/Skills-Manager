import assert from "node:assert/strict";
import { test } from "node:test";

import { confirmSharedOperationPreviews, formatSkillBindingImpacts } from "./skillOperationConfirmation.ts";

const impact = (provider_id: string, root_path?: string | null) => ({
  provider_id,
  display_name: "Codex",
  root_path,
  shared: true,
});
const preview = (requires_confirmation: boolean, impacts = [impact("codex", "/project/.agents/skills")]) => ({
  skill_instance_id: "project:example:skill",
  artifact_id: "skill",
  provider_id: "codex",
  scope: "project" as const,
  action: "enable" as const,
  impacts,
  requires_confirmation,
});

test("impact display deduplicates by provider and root while retaining distinct roots and providers", () => {
  assert.equal(formatSkillBindingImpacts([
    impact("codex", "/one/.agents/skills"),
    impact("codex", "/one/.agents/skills"),
    impact("codex", "/two/.agents/skills"),
    { ...impact("claude", "/one/.agents/skills"), display_name: "Claude" },
    impact("legacy"),
    impact("legacy", null),
  ]), "Codex (/one/.agents/skills)\nCodex (/two/.agents/skills)\nClaude (/one/.agents/skills)\nCodex");
});

test("nonshared operations never receive shared consent or show a dialog", async () => {
  const unexpectedConfirmation = async () => { throw new Error("unexpected dialog"); };
  assert.equal(await confirmSharedOperationPreviews([], unexpectedConfirmation, "Confirm"), false);
  assert.equal(await confirmSharedOperationPreviews([preview(false)], unexpectedConfirmation, "Confirm"), false);
});

test("shared consent requires acceptance of all deduplicated preview impacts", async () => {
  let displayed = "";
  const confirmed = await confirmSharedOperationPreviews([
    { ...preview(true), warning: "Shared changes" },
    { ...preview(true, [impact("codex", "/second/.agents/skills")]), warning: "Shared changes" },
    preview(true),
  ], async (message) => { displayed = message; return true; }, "Confirm");
  assert.equal(confirmed, true);
  assert.equal(displayed, "Shared changes\nConfirm\n\nCodex (/project/.agents/skills)\nCodex (/second/.agents/skills)");
});

test("declining shared confirmation cancels the operation instead of granting unshared consent", async () => {
  assert.equal(await confirmSharedOperationPreviews([preview(true)], async () => false, "Confirm"), null);
});

test("a preview or dialog failure cannot grant shared consent", async () => {
  await assert.rejects(confirmSharedOperationPreviews([preview(true)], async () => {
    throw new Error("dialog unavailable");
  }, "Confirm"), /dialog unavailable/);
});
