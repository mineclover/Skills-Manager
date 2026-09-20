import assert from "node:assert/strict";
import test from "node:test";
import type { MarketplaceInstallation, MarketplaceSkill } from "../../types/index.ts";
import {
  countMarketplaceUpdateInstallations,
  getMarketplaceUpdateCandidates,
} from "./marketplaceUpdates.ts";

function makeSkill(
  id: string,
  installations: MarketplaceInstallation[],
): MarketplaceSkill {
  return {
    id,
    name: id,
    description: null,
    author: null,
    source_id: "source",
    source_name: "Source",
    repo_url: null,
    skill_path: null,
    external_url: null,
    tags: [],
    install_status: installations.some(
      (installation) => installation.install_status === "update_available",
    ) ? "update_available" : "installed",
    installations,
  };
}

const globalUpdate: MarketplaceInstallation = {
  instance_id: "global:demo",
  scope: "global",
  tool_ids: [],
  install_status: "update_available",
};

test("groups update installations by skill", () => {
  const projectUpdate: MarketplaceInstallation = {
    ...globalUpdate,
    instance_id: "project:alpha:demo",
    scope: "project",
    project_id: "alpha",
    project_name: "Alpha",
  };
  const installed: MarketplaceInstallation = {
    ...globalUpdate,
    instance_id: "global:current",
    install_status: "installed",
  };
  const candidates = getMarketplaceUpdateCandidates([
    makeSkill("demo", [globalUpdate, projectUpdate]),
    makeSkill("current", [installed]),
  ]);

  assert.deepEqual(candidates.map((candidate) => candidate.skill.id), ["demo"]);
  assert.deepEqual(
    candidates[0]?.installations.map((installation) => installation.instance_id),
    ["global:demo", "project:alpha:demo"],
  );
  assert.equal(countMarketplaceUpdateInstallations(candidates), 2);
});
