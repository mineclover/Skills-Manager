import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "@/i18n";
import {
  getActionableTargets,
  getInstallActionForTarget,
  getTargetsFromSelection,
  hasInvalidProjectTarget,
} from "@/pages/marketplace/installTargets";
import type {
  MarketplaceInstallSelection,
  MarketplaceSkill,
  ProjectBinding,
  Tool,
} from "@/types";
import { MODAL_LAYER_Z_INDEX, MODAL_OVERLAY_COLOR } from "@/constants/modal";
import { InstallTargetPicker } from "@/components/marketplace/InstallTargetPicker";

interface InstallTargetDialogProps {
  skill: MarketplaceSkill | null;
  projects: ProjectBinding[];
  activeProjectId?: string | null;
  projectTools: Tool[];
  installing: boolean;
  onClose: () => void;
  onSubmit: (skill: MarketplaceSkill, selection: MarketplaceInstallSelection) => void;
  onManageProjects: () => void;
}

function submitLabel(
  skill: MarketplaceSkill,
  selection: MarketplaceInstallSelection,
  installing: boolean,
  t: ReturnType<typeof useTranslation>["t"],
) {
  if (installing) return t("marketplace.installing");
  const actions = getTargetsFromSelection(selection).map(
    (target) => getInstallActionForTarget(skill, target),
  );
  if (actions.includes("install")) return t("marketplace.installSelectedTargets");
  if (actions.includes("update")) return t("marketplace.updateSelectedTargets");
  return t("marketplace.installed");
}

export function InstallTargetDialog({
  skill,
  projects,
  activeProjectId = null,
  projectTools,
  installing,
  onClose,
  onSubmit,
  onManageProjects,
}: InstallTargetDialogProps) {
  const { t } = useTranslation();
  const [selection, setSelection] = useState<MarketplaceInstallSelection>({
    global: true,
    projects: [],
  });
  const skillId = skill?.id;

  useEffect(() => {
    if (skillId) setSelection({ global: true, projects: [] });
  }, [skillId]);

  useEffect(() => {
    if (!skill) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !installing) onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [installing, onClose, skill]);

  const submitDisabled = useMemo(() => {
    if (!skill || installing || hasInvalidProjectTarget(selection)) return true;
    return getActionableTargets(skill, selection).length === 0;
  }, [installing, selection, skill]);

  if (!skill) return null;

  return (
    <div
      role="presentation"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: MODAL_LAYER_Z_INDEX,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: "24px",
        backgroundColor: MODAL_OVERLAY_COLOR,
      }}
      onClick={() => {
        if (!installing) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="install-target-title"
        style={{
          width: "min(620px, calc(100vw - 32px))",
          maxHeight: "calc(100vh - 48px)",
          overflow: "auto",
          padding: "22px",
          border: "1px solid var(--border)",
          borderRadius: "8px",
          backgroundColor: "var(--background)",
          boxShadow: "var(--shadow-xl)",
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <div style={{ marginBottom: "18px" }}>
          <h2 id="install-target-title" style={{ margin: 0, fontSize: "16px", fontWeight: 700 }}>
            {t("marketplace.installTargetTitle")}
          </h2>
          <p style={{ margin: "6px 0 0", fontSize: "13px", lineHeight: 1.5, color: "var(--muted-foreground)" }}>
            {t("marketplace.installTargetDesc").replace("{name}", skill.name)}
          </p>
        </div>

        <InstallTargetPicker
          skill={skill}
          projects={projects}
          activeProjectId={activeProjectId}
          tools={projectTools}
          selection={selection}
          disabled={installing}
          onChange={setSelection}
          onManageProjects={onManageProjects}
        />

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "10px", marginTop: "20px" }}>
          <button type="button" onClick={onClose} disabled={installing} style={{ height: "36px", padding: "0 14px", border: "1px solid var(--border)", borderRadius: "8px", backgroundColor: "var(--secondary)", color: "var(--foreground)", cursor: installing ? "wait" : "pointer" }}>
            {t("common.cancel")}
          </button>
          <button
            type="button"
            onClick={() => onSubmit(skill, selection)}
            disabled={submitDisabled}
            style={{ minWidth: "116px", height: "36px", padding: "0 16px", border: 0, borderRadius: "8px", backgroundColor: "var(--primary)", color: "var(--primary-foreground)", fontWeight: 650, cursor: submitDisabled ? "not-allowed" : "pointer", opacity: submitDisabled ? 0.55 : 1 }}
          >
            {submitLabel(skill, selection, installing, t)}
          </button>
        </div>
      </div>
    </div>
  );
}
