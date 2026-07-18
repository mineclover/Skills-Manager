import { test } from "node:test";
import assert from "node:assert/strict";
import { getDetectedToolIds } from "./getEnabledToolIds.ts";

test("getDetectedToolIds keeps detected tools even when manager activation is off", () => {
  const toolIds = getDetectedToolIds([
    { id: "cursor", config: { enabled: false, detected: true } },
    { id: "codex", config: { enabled: true, detected: true } },
    { id: "claude-code", config: { enabled: true, detected: true } },
  ]);

  assert.deepEqual(toolIds, ["claude-code", "codex", "cursor"]);
});

test("getDetectedToolIds returns an empty list when no tools are detected", () => {
  const toolIds = getDetectedToolIds([
    { id: "cursor", config: { enabled: false, detected: false } },
    { id: "codex", config: { enabled: false, detected: false } },
  ]);

  assert.deepEqual(toolIds, []);
});
