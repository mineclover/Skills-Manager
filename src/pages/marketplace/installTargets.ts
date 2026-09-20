import type {
  MarketplaceInstallation,
  MarketplaceInstallSelection,
  MarketplaceInstallStatus,
  MarketplaceInstallTarget,
  MarketplaceSkill,
  ProjectBinding,
} from "@/types";

export type MarketplaceInstallAction = "install" | "update" | "installed";

export function getTargetsFromSelection(
  selection: MarketplaceInstallSelection,
): MarketplaceInstallTarget[] {
  const targets: MarketplaceInstallTarget[] = selection.global
    ? [{ scope: "global", tool_ids: [] }]
    : [];
  const seenProjects = new Set<string>();
  for (const target of selection.projects) {
    const projectId = target.project_id?.trim();
    if (target.scope !== "project" || !projectId || seenProjects.has(projectId)) continue;
    seenProjects.add(projectId);
    targets.push({
      scope: "project",
      project_id: projectId,
      tool_ids: Array.from(new Set(target.tool_ids ?? [])),
    });
  }
  return targets;
}

export function getActionableTargets(
  skill: MarketplaceSkill,
  selection: MarketplaceInstallSelection,
): MarketplaceInstallTarget[] {
  return getTargetsFromSelection(selection).filter(
    (target) => getInstallActionForTarget(skill, target) !== "installed",
  );
}

export function hasInvalidProjectTarget(selection: MarketplaceInstallSelection): boolean {
  return selection.projects.some(
    (target) => target.scope !== "project"
      || !target.project_id?.trim()
      || (target.tool_ids?.length ?? 0) === 0,
  );
}

export function getInstallationForTarget(
  skill: MarketplaceSkill,
  target: MarketplaceInstallTarget,
): MarketplaceInstallation | null {
  return (skill.installations ?? []).find((installation) => {
    if (installation.scope !== target.scope) return false;
    if (target.scope === "global") return true;
    return installation.project_id === target.project_id;
  }) ?? null;
}

export function getInstallStatusForTarget(
  skill: MarketplaceSkill,
  target: MarketplaceInstallTarget,
): MarketplaceInstallStatus {
  return getInstallationForTarget(skill, target)?.install_status ?? "not_installed";
}

export function getInstallActionForTarget(
  skill: MarketplaceSkill,
  target: MarketplaceInstallTarget,
): MarketplaceInstallAction {
  const status = getInstallStatusForTarget(skill, target);
  if (status === "update_available") return "update";
  if (status === "installed") {
    const installation = getInstallationForTarget(skill, target);
    if (target.scope === "project" && (target.tool_ids?.length ?? 0) > 0) {
      const selectedTools = new Set(target.tool_ids ?? []);
      const installedTools = new Set(installation?.tool_ids ?? []);
      if (
        selectedTools.size !== installedTools.size
        || Array.from(selectedTools).some((toolId) => !installedTools.has(toolId))
      ) {
        return "install";
      }
    }
    return "installed";
  }
  return "install";
}

export function aggregateMarketplaceInstallStatus(
  installations: MarketplaceInstallation[],
): MarketplaceInstallStatus {
  if (installations.some((item) => item.install_status === "update_available")) {
    return "update_available";
  }
  if (installations.some((item) => item.install_status === "installed")) {
    return "installed";
  }
  return "not_installed";
}

export function getMarketplacePrimaryAction(
  skill: MarketplaceSkill,
  projects: ProjectBinding[],
): MarketplaceInstallAction {
  const targets: MarketplaceInstallTarget[] = [{ scope: "global" }];
  for (const project of projects) {
    targets.push({ scope: "project", project_id: project.id });
  }
  const actions = targets.map((target) => getInstallActionForTarget(skill, target));
  if (actions.includes("update")) return "update";
  if (actions.includes("install")) return "install";
  return "installed";
}

export function getUninstallTargets(skill: MarketplaceSkill): MarketplaceInstallation[] {
  return (skill.installations ?? []).filter((installation) => (
    installation.install_status === "installed"
    || installation.install_status === "update_available"
  ));
}
