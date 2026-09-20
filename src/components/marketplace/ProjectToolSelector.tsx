import { Check, FolderKanban } from "lucide-react";
import { useTranslation } from "@/i18n";
import type { ProjectBinding, Tool } from "@/types";
import {
  groupProjectToolsByDirectory,
  toggleProjectToolGroupSelection,
} from "@/pages/marketplace/projectToolTargets";

interface ProjectToolSelectorProps {
  project: ProjectBinding | null;
  tools: Tool[];
  selectedToolIds: string[];
  disabled?: boolean;
  onChange: (toolIds: string[]) => void;
}

function resolveProjectToolPath(project: ProjectBinding, tool: Tool): string {
  const relative = tool.project_skills_dir?.replace(/^[/\\]+/, "") ?? "";
  const root = project.root_path ?? project.skills_dir;
  return relative ? `${root.replace(/[\\/]+$/, "")}/${relative}` : root;
}

export function ProjectToolSelector({
  project,
  tools,
  selectedToolIds,
  disabled = false,
  onChange,
}: ProjectToolSelectorProps) {
  const { t } = useTranslation();
  const toolGroups = groupProjectToolsByDirectory(tools);

  if (!project) return null;

  const toggleToolGroup = (groupToolIds: string[]) => {
    if (disabled) return;
    onChange(toggleProjectToolGroupSelection(selectedToolIds, groupToolIds));
  };

  return (
    <div style={{ display: "grid", gap: "8px", marginTop: "12px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "7px", fontSize: "12px", fontWeight: 650, color: "var(--foreground)" }}>
        <FolderKanban size={14} />
        {t("marketplace.projectToolTargets")}
      </div>
      <div style={{ fontSize: "11px", lineHeight: 1.45, color: "var(--muted-foreground)" }}>
        {t("marketplace.projectToolTargetsDesc")}
      </div>
      {toolGroups.length === 0 ? (
        <div style={{ padding: "10px 12px", border: "1px solid var(--color-warning-border)", borderRadius: "8px", backgroundColor: "var(--color-warning-bg)", color: "var(--color-warning)", fontSize: "11px", lineHeight: 1.45 }}>
          {t("marketplace.noProjectToolTargets")}
        </div>
      ) : (
        <div role="group" aria-label={t("marketplace.projectToolTargets")} style={{ display: "grid", gap: "6px" }}>
          {toolGroups.map((group) => {
            const groupToolIds = group.tools.map((tool) => tool.id);
            const selected = groupToolIds.some((toolId) => selectedToolIds.includes(toolId));
            const path = resolveProjectToolPath(project, group.tools[0]);
            return (
              <label
                key={group.relativePath}
                title={path}
                style={{ display: "grid", gridTemplateColumns: "18px minmax(0, 1fr)", alignItems: "center", gap: "8px", minHeight: "42px", padding: "7px 9px", border: `1px solid ${selected ? "var(--primary)" : "var(--border)"}`, borderRadius: "8px", backgroundColor: selected ? "var(--primary-tint)" : "var(--secondary)", cursor: disabled ? "not-allowed" : "pointer", opacity: disabled ? 0.65 : 1 }}
              >
                <input
                  type="checkbox"
                  checked={selected}
                  disabled={disabled}
                  onChange={() => toggleToolGroup(groupToolIds)}
                  style={{ width: "15px", height: "15px", accentColor: "var(--primary)" }}
                />
                <span style={{ minWidth: 0 }}>
                  <span style={{ display: "flex", alignItems: "center", gap: "5px", fontSize: "12px", fontWeight: 650, color: "var(--foreground)" }}>
                    {selected && <Check size={13} color="var(--primary)" />}
                    {group.tools.map((tool) => tool.name).join(" / ")}
                  </span>
                  <span style={{ display: "block", marginTop: "2px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "10px", color: "var(--muted-foreground)" }}>
                    {path}
                  </span>
                </span>
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}
