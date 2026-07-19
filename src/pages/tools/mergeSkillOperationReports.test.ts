import assert from "node:assert/strict";
import { test } from "node:test";

import { mergeSkillOperationReports } from "./mergeSkillOperationReports.ts";

const report = (overrides: Partial<Parameters<typeof mergeSkillOperationReports>[0][number]> = {}) => ({
  operation_id: "operation",
  action: "enable" as const,
  scope: "global" as const,
  project_id: null,
  provider_id: "codex",
  requested_count: 1,
  attempted_count: 1,
  applied_count: 1,
  skipped_count: 0,
  failed_count: 0,
  failures: [],
  impacts: [],
  completed_at: 1,
  ...overrides,
});

test("mergeSkillOperationReports adds counts and preserves failures and impacts", () => {
  const merged = mergeSkillOperationReports([
    report({ operation_id: "one", applied_count: 1, completed_at: 2 }),
    report({
      operation_id: "two",
      applied_count: 0,
      skipped_count: 1,
      failed_count: 1,
      failures: [{ skill_instance_id: "global:broken", provider_id: "codex", message: "broken target" }],
      impacts: [{ provider_id: "agents-directory", display_name: "Shared Agents", shared: true }],
      completed_at: 3,
    }),
  ]);

  assert.equal(merged?.operation_id, "one,two");
  assert.equal(merged?.requested_count, 2);
  assert.equal(merged?.applied_count, 1);
  assert.equal(merged?.skipped_count, 1);
  assert.equal(merged?.failed_count, 1);
  assert.equal(merged?.failures[0]?.message, "broken target");
  assert.equal(merged?.impacts[0]?.provider_id, "agents-directory");
  assert.equal(merged?.completed_at, 3);
});

test("mergeSkillOperationReports returns null for an empty batch", () => {
  assert.equal(mergeSkillOperationReports([]), null);
});
