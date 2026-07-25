import test from "node:test";
import assert from "node:assert/strict";
import { buildSkillsHeaderActionLayout } from "./headerActionLayout.ts";

test("buildSkillsHeaderActionLayout keeps all normal-mode actions in the more menu", () => {
  assert.deepEqual(buildSkillsHeaderActionLayout(false), {
    primaryActionIds: [],
    moreActionIds: [
      "batch-manage",
      "project-bindings",
      "scan-import",
      "import-skills",
      "export-skills",
    ],
    secondaryActionIds: ["create-skill"],
  });
});

test("buildSkillsHeaderActionLayout surfaces export-skills as a primary action in batch mode", () => {
  assert.deepEqual(buildSkillsHeaderActionLayout(true), {
    primaryActionIds: ["batch-manage", "batch-configure", "export-skills"],
    moreActionIds: [],
    secondaryActionIds: [],
  });
});
