import { FolderKanban, Globe2, Settings2 } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "@/i18n";
import {
  getInstallStatusForTarget,
  getInstallationForTarget,
} from "@/pages/marketplace/installTargets";
import { groupProjectToolsByDirectory } from "@/pages/marketplace/projectToolTargets";
import type {
  MarketplaceInstallSelection,
  MarketplaceInstallStatus,
  MarketplaceInstallTarget,
  MarketplaceSkill,
  ProjectBinding,
  Tool,
} from "@/types";
import { ProjectToolSelector } from "@/components/marketplace/ProjectToolSelector";

interface InstallTargetPickerProps {
  skill?: MarketplaceSkill | null;
  projects: ProjectBinding[];
  activeProjectId?: string | null;
  tools: Tool[];
  selection: MarketplaceInstallSelection;
  disabled?: boolean;
  onChange: (selection: MarketplaceInstallSelection) => void;
  onManageProjects?: () => void;
}

function statusTranslationKey(status: MarketplaceInstallStatus) {
  if (status === "installed") return "marketplace.targetStatusInstalled" as const;
  if (status === "update_available") return "marketplace.targetStatusUpdateAvailable" as const;
  return "marketplace.targetStatusNotInstalled" as const;
}

function projectTarget(projectId: string, toolIds: string[] = []): MarketplaceInstallTarget {
  return { scope: "project", project_id: projectId, tool_ids: toolIds };
}

export function InstallTargetPicker({
  skill = null,
  projects,
  activeProjectId = null,
  tools,
  selection,
  disabled = false,
  onChange,
  onManageProjects,
}: InstallTargetPickerProps) {
  const { t } = useTranslation();
  const defaultProjectToolIds = useMemo(
    () => groupProjectToolsByDirectory(tools).flatMap((group) => (
      group.tools.map((tool) => tool.id)
    )),
    [tools],
  );

  const toggleProject = (project: ProjectBinding) => {
    if (disabled || !project.root_path) return;
    const existingTarget = selection.projects.find(
      (target) => target.project_id === project.id,
    );
    if (existingTarget) {
      onChange({
        ...selection,
        projects: selection.projects.filter((target) => target.project_id !== project.id),
      });
      return;
    }

    const installation = skill
      ? getInstallationForTarget(skill, projectTarget(project.id))
      : null;
    const toolIds = installation?.tool_ids.length
      ? installation.tool_ids
      : defaultProjectToolIds;
    onChange({
      ...selection,
      projects: [...selection.projects, projectTarget(project.id, toolIds)],
    });
  };

  const updateProjectTools = (projectId: string, toolIds: string[]) => {
    onChange({
      ...selection,
      projects: selection.projects.map((target) => (
        target.project_id === projectId ? { ...target, tool_ids: toolIds } : target
      )),
    });
  };

  return (
    <div style={{ display: "grid", gap: "10px" }}>
      <label
        style={{
          display: "grid",
          gridTemplateColumns: "20px 32px minmax(0, 1fr) auto",
          alignItems: "center",
          gap: "10px",
          minHeight: "62px",
          padding: "10px 12px",
          border: `1px solid ${selection.global ? "var(--primary)" : "var(--border)"}`,
          borderRadius: "8px",
          backgroundColor: selection.global ? "var(--primary-tint)" : "var(--secondary)",
          cursor: disabled ? "not-allowed" : "pointer",
          opacity: disabled ? 0.65 : 1,
        }}
      >
        <input
          type="checkbox"
          checked={selection.global}
          disabled={disabled}
          onChange={() => onChange({ ...selection, global: !selection.global })}
          style={{ width: "16px", height: "16px", accentColor: "var(--primary)" }}
        />
        <span style={{ width: "32px", height: "32px", display: "inline-flex", alignItems: "center", justifyContent: "center", color: selection.global ? "var(--primary)" : "var(--muted-foreground)" }}>
          <Globe2 size={18} />
        </span>
        <span style={{ minWidth: 0 }}>
          <span style={{ display: "block", fontSize: "13px", fontWeight: 650, color: "var(--foreground)" }}>
            {t("marketplace.targetGlobal")}
          </span>
          <span style={{ display: "block", marginTop: "3px", fontSize: "11px", color: "var(--muted-foreground)" }}>
            {t("marketplace.targetGlobalDesc")}
          </span>
        </span>
        {skill ? (
          <span style={{ padding: "3px 7px", borderRadius: "6px", border: "1px solid var(--border)", fontSize: "10px", fontWeight: 600, whiteSpace: "nowrap" }}>
            {t(statusTranslationKey(getInstallStatusForTarget(skill, { scope: "global" })))}
          </span>
        ) : null}
      </label>

      <div style={{ display: "flex", alignItems: "center", gap: "7px", marginTop: "2px", fontSize: "12px", fontWeight: 650, color: "var(--foreground)" }}>
        <FolderKanban size={14} />
        {t("marketplace.projectTargets")}
      </div>

      {projects.length === 0 ? (
        <div style={{ padding: "12px", border: "1px solid var(--border)", borderRadius: "8px", color: "var(--muted-foreground)", fontSize: "12px" }}>
          {t("marketplace.noProjectBindings")}
        </div>
      ) : (
        <div style={{ display: "grid", gap: "8px" }}>
          {projects.map((project) => {
            const target = selection.projects.find((item) => item.project_id === project.id);
            const selected = Boolean(target);
            const projectReady = Boolean(project.root_path);
            const status = skill
              ? getInstallStatusForTarget(skill, projectTarget(project.id))
              : null;
            return (
              <div key={project.id} style={{ border: `1px solid ${selected ? "var(--primary)" : "var(--border)"}`, borderRadius: "8px", backgroundColor: selected ? "var(--primary-tint)" : "var(--secondary)", overflow: "hidden", opacity: projectReady ? 1 : 0.62 }}>
                <label
                  title={projectReady ? project.root_path ?? undefined : t("marketplace.projectBindingNeedsRoot")}
                  style={{
                    display: "grid",
                    gridTemplateColumns: "20px 32px minmax(0, 1fr) auto",
                    alignItems: "center",
                    gap: "10px",
                    minHeight: "62px",
                    padding: "10px 12px",
                    cursor: disabled || !projectReady ? "not-allowed" : "pointer",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selected}
                    disabled={disabled || !projectReady}
                    onChange={() => toggleProject(project)}
                    style={{ width: "16px", height: "16px", accentColor: "var(--primary)" }}
                  />
                  <span style={{ width: "32px", height: "32px", display: "inline-flex", alignItems: "center", justifyContent: "center", color: selected ? "var(--primary)" : "var(--muted-foreground)" }}>
                    <FolderKanban size={18} />
                  </span>
                  <span style={{ minWidth: 0 }}>
                    <span style={{ display: "flex", alignItems: "center", gap: "7px", minWidth: 0 }}>
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "13px", fontWeight: 650, color: "var(--foreground)" }}>
                        {project.name}
                      </span>
                      {project.id === activeProjectId ? (
                        <span style={{ flexShrink: 0, padding: "2px 5px", borderRadius: "4px", backgroundColor: "var(--background)", color: "var(--muted-foreground)", fontSize: "9px", fontWeight: 650 }}>
                          {t("marketplace.currentProjectBadge")}
                        </span>
                      ) : null}
                    </span>
                    <span style={{ display: "block", marginTop: "3px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "11px", color: "var(--muted-foreground)" }}>
                      {project.root_path ?? t("marketplace.projectBindingNeedsRoot")}
                    </span>
                  </span>
                  {status ? (
                    <span style={{ padding: "3px 7px", borderRadius: "6px", border: "1px solid var(--border)", fontSize: "10px", fontWeight: 600, whiteSpace: "nowrap" }}>
                      {t(statusTranslationKey(status))}
                    </span>
                  ) : null}
                </label>

                {selected && target ? (
                  <div style={{ padding: "0 12px 12px 42px", borderTop: "1px solid var(--border)" }}>
                    <ProjectToolSelector
                      project={project}
                      tools={tools}
                      selectedToolIds={target.tool_ids ?? []}
                      disabled={disabled}
                      onChange={(toolIds) => updateProjectTools(project.id, toolIds)}
                    />
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      )}

      {onManageProjects ? (
        <button
          type="button"
          onClick={onManageProjects}
          disabled={disabled}
          style={{ justifySelf: "start", display: "inline-flex", alignItems: "center", gap: "6px", padding: 0, border: 0, background: "transparent", color: "var(--primary)", fontSize: "12px", cursor: disabled ? "not-allowed" : "pointer" }}
        >
          <Settings2 size={14} />
          {t("marketplace.manageProjectBindings")}
        </button>
      ) : null}
    </div>
  );
}
