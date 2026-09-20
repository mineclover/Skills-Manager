import assert from "node:assert/strict";
import test from "node:test";
import type { Tool } from "../../types/index.ts";
import {
  groupProjectToolsByDirectory,
  toggleProjectToolGroupSelection,
} from "./projectToolTargets.ts";

function tool(id: string, projectSkillsDir: string | null, enabled = true): Tool {
  return {
    id,
    name: id,
    detected: true,
    cli_available: true,
    source: "builtin",
    project_skills_dir: projectSkillsDir,
    config: {
      enabled,
      detected: true,
      skills_path: `/global/${id}/skills`,
      config_path: `/global/${id}`,
    },
  };
}

test("groups enabled tools that share the same physical project directory", () => {
  const groups = groupProjectToolsByDirectory([
    tool("codex", ".agents/skills"),
    tool("vercel-skills", ".agents/skills"),
    tool("claude-code", ".claude/skills"),
    tool("disabled", ".disabled/skills", false),
    tool("unsupported", null),
  ]);

  assert.deepEqual(
    groups.map((group) => ({
      relativePath: group.relativePath,
      toolIds: group.tools.map((item) => item.id),
    })),
    [
      { relativePath: ".agents/skills", toolIds: ["codex", "vercel-skills"] },
      { relativePath: ".claude/skills", toolIds: ["claude-code"] },
    ],
  );
});

test("toggles every tool sharing a project directory as one selection", () => {
  assert.deepEqual(
    toggleProjectToolGroupSelection(["claude-code"], ["codex", "vercel-skills"]),
    ["claude-code", "codex", "vercel-skills"],
  );
  assert.deepEqual(
    toggleProjectToolGroupSelection(
      ["claude-code", "codex", "vercel-skills"],
      ["codex", "vercel-skills"],
    ),
    ["claude-code"],
  );
});
