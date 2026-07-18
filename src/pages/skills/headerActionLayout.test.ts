import test from "node:test";
import assert from "node:assert/strict";
import { buildSkillsHeaderActionLayout } from "./headerActionLayout.ts";

test("buildSkillsHeaderActionLayout puts secondary actions in the more menu in normal mode", () => {
  assert.deepEqual(buildSkillsHeaderActionLayout(false), {
    primaryActionIds: [],
    moreActionIds: ["batch-manage", "project-bindings", "scan-import"],
    secondaryActionIds: ["create-skill"],
  });
});

test("buildSkillsHeaderActionLayout keeps only batch actions in batch mode", () => {
  assert.deepEqual(buildSkillsHeaderActionLayout(true), {
    primaryActionIds: ["batch-manage", "batch-configure"],
    moreActionIds: [],
    secondaryActionIds: [],
  });
});
