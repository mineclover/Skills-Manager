import assert from "node:assert/strict";
import test from "node:test";
import type { MarketplaceInstallation, MarketplaceSkill } from "../../types/index.ts";
import {
  aggregateMarketplaceInstallStatus,
  getActionableTargets,
  getInstallActionForTarget,
  getInstallStatusForTarget,
  getMarketplacePrimaryAction,
  getTargetsFromSelection,
  hasInvalidProjectTarget,
  getUninstallTargets,
} from "./installTargets.ts";

function makeSkill(installations: MarketplaceInstallation[]): MarketplaceSkill {
  return {
    id: "source::demo",
    name: "Demo",
    description: null,
    author: null,
    source_id: "source",
    source_name: "Source",
    repo_url: null,
    skill_path: null,
    external_url: null,
    tags: [],
    install_status: aggregateMarketplaceInstallStatus(installations),
    installations,
  };
}

const globalInstalled: MarketplaceInstallation = {
  instance_id: "global:demo",
  scope: "global",
  tool_ids: [],
  install_status: "installed",
};
const projectOutdated: MarketplaceInstallation = {
  instance_id: "project:alpha:demo",
  scope: "project",
  project_id: "alpha",
  project_name: "Alpha",
  tool_ids: ["claude-code"],
  install_status: "update_available",
};

test("maps each target to its own status and action", () => {
  const skill = makeSkill([globalInstalled]);
  assert.equal(getInstallStatusForTarget(skill, { scope: "global" }), "installed");
  assert.equal(getInstallActionForTarget(skill, { scope: "global" }), "installed");
  assert.equal(
    getInstallActionForTarget(skill, { scope: "project", project_id: "alpha" }),
    "install",
  );
});

test("aggregate status prioritizes updates and primary action considers both scopes", () => {
  const skill = makeSkill([globalInstalled, projectOutdated]);
  assert.equal(skill.install_status, "update_available");
  assert.equal(
    getMarketplacePrimaryAction(skill, [{ id: "alpha", name: "Alpha", skills_dir: "/tmp" }]),
    "update",
  );
});

test("primary action includes update state from a non-active project", () => {
  const projectBetaOutdated = {
    ...projectOutdated,
    instance_id: "project:beta:demo",
    project_id: "beta",
    project_name: "Beta",
  };
  const skill = makeSkill([globalInstalled, projectBetaOutdated]);

  assert.equal(
    getMarketplacePrimaryAction(skill, [
      { id: "alpha", name: "Alpha", skills_dir: "/alpha" },
      { id: "beta", name: "Beta", skills_dir: "/beta" },
    ]),
    "update",
  );
});

test("uninstall targets preserve distinct instance ids", () => {
  const targets = getUninstallTargets(makeSkill([globalInstalled, projectOutdated]));
  assert.deepEqual(targets.map((item) => item.instance_id), [
    "global:demo",
    "project:alpha:demo",
  ]);
});

test("offers installation when an installed project target is missing a selected tool link", () => {
  const installedProject = { ...projectOutdated, install_status: "installed" as const };
  const skill = makeSkill([installedProject]);

  assert.equal(
    getInstallActionForTarget(skill, {
      scope: "project",
      project_id: "alpha",
      tool_ids: ["claude-code", "codex"],
    }),
    "install",
  );
  assert.equal(
    getInstallActionForTarget(skill, {
      scope: "project",
      project_id: "alpha",
      tool_ids: ["claude-code"],
    }),
    "installed",
  );
});

test("offers installation when removing an existing project tool link", () => {
  const installedProject = {
    ...projectOutdated,
    install_status: "installed" as const,
    tool_ids: ["claude-code", "gemini"],
  };

  assert.equal(
    getInstallActionForTarget(makeSkill([installedProject]), {
      scope: "project",
      project_id: "alpha",
      tool_ids: ["claude-code"],
    }),
    "install",
  );
});

test("expands one multi-location selection into global and distinct project targets", () => {
  assert.deepEqual(
    getTargetsFromSelection({
      global: true,
      projects: [
        { scope: "project", project_id: "alpha", tool_ids: ["claude-code"] },
        { scope: "project", project_id: "beta", tool_ids: ["codex", "codex"] },
        { scope: "project", project_id: "alpha", tool_ids: ["gemini"] },
      ],
    }),
    [
      { scope: "global", tool_ids: [] },
      { scope: "project", project_id: "alpha", tool_ids: ["claude-code"] },
      { scope: "project", project_id: "beta", tool_ids: ["codex"] },
    ],
  );
});

test("batch selection skips already installed targets and validates project tools", () => {
  const skill = makeSkill([globalInstalled]);
  const selection = {
    global: true,
    projects: [
      { scope: "project" as const, project_id: "alpha", tool_ids: ["claude-code"] },
    ],
  };

  assert.deepEqual(getActionableTargets(skill, selection), [selection.projects[0]]);
  assert.equal(hasInvalidProjectTarget(selection), false);
  assert.equal(
    hasInvalidProjectTarget({
      global: false,
      projects: [{ scope: "project", project_id: "alpha", tool_ids: [] }],
    }),
    true,
  );
});
