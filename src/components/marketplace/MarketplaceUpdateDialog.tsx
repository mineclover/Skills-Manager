import { useEffect } from "react";
import {
  FolderKanban,
  Globe2,
  RefreshCw,
  TriangleAlert,
  X,
} from "lucide-react";
import { useTranslation } from "@/i18n";
import type { MarketplaceUpdateCandidate } from "@/pages/marketplace/marketplaceUpdates";
import { countMarketplaceUpdateInstallations } from "@/pages/marketplace/marketplaceUpdates";
import { MODAL_LAYER_Z_INDEX, MODAL_OVERLAY_COLOR } from "@/constants/modal";

interface MarketplaceUpdateDialogProps {
  open: boolean;
  candidates: MarketplaceUpdateCandidate[];
  updating: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export function MarketplaceUpdateDialog({
  open,
  candidates,
  updating,
  onClose,
  onConfirm,
}: MarketplaceUpdateDialogProps) {
  const { t } = useTranslation();
  const installationCount = countMarketplaceUpdateInstallations(candidates);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !updating) onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, open, updating]);

  if (!open) return null;

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
        if (!updating) onClose();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="marketplace-update-dialog-title"
        style={{
          width: "min(620px, calc(100vw - 32px))",
          maxHeight: "min(720px, calc(100vh - 48px))",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          border: "1px solid var(--border)",
          borderRadius: "8px",
          backgroundColor: "var(--background)",
          boxShadow: "var(--shadow-xl)",
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <header style={{ display: "flex", alignItems: "flex-start", gap: "14px", padding: "20px 20px 16px" }}>
          <span
            style={{
              width: "36px",
              height: "36px",
              flexShrink: 0,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "8px",
              color: "var(--primary)",
              backgroundColor: "var(--primary-tint)",
              border: "1px solid var(--primary-tint-border)",
            }}
          >
            <RefreshCw size={17} />
          </span>
          <span style={{ minWidth: 0, flex: 1 }}>
            <h2 id="marketplace-update-dialog-title" style={{ margin: 0, fontSize: "16px", lineHeight: 1.35, fontWeight: 700, color: "var(--foreground)" }}>
              {t("marketplace.updateDialogTitle").replace("{count}", String(candidates.length))}
            </h2>
            <span style={{ display: "block", marginTop: "5px", fontSize: "12px", lineHeight: 1.5, color: "var(--muted-foreground)" }}>
              {t("marketplace.updateDialogSummary")
                .replace("{skills}", String(candidates.length))
                .replace("{locations}", String(installationCount))}
            </span>
          </span>
          <button
            type="button"
            onClick={onClose}
            disabled={updating}
            title={t("shortcuts.close")}
            aria-label={t("shortcuts.close")}
            style={{
              width: "32px",
              height: "32px",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
              padding: 0,
              border: "1px solid transparent",
              borderRadius: "6px",
              color: "var(--muted-foreground)",
              backgroundColor: "transparent",
              cursor: updating ? "not-allowed" : "pointer",
              opacity: updating ? 0.5 : 1,
            }}
          >
            <X size={16} />
          </button>
        </header>

        <div
          style={{
            margin: "0 20px 14px",
            display: "flex",
            alignItems: "flex-start",
            gap: "9px",
            padding: "10px 12px",
            border: "1px solid var(--primary-tint-border)",
            borderRadius: "7px",
            color: "var(--foreground)",
            backgroundColor: "var(--primary-tint)",
            fontSize: "12px",
            lineHeight: 1.5,
          }}
        >
          <TriangleAlert size={15} style={{ flexShrink: 0, marginTop: "1px", color: "var(--primary)" }} />
          <span>{t("marketplace.updateDialogOverwriteWarning")}</span>
        </div>

        <div style={{ minHeight: 0, overflowY: "auto", borderTop: "1px solid var(--border)", borderBottom: "1px solid var(--border)" }}>
          {candidates.length === 0 ? (
            <div style={{ padding: "28px 20px", textAlign: "center", fontSize: "13px", color: "var(--muted-foreground)" }}>
              {t("marketplace.noUpdates")}
            </div>
          ) : candidates.map(({ skill, installations }, index) => (
            <article
              key={skill.id}
              style={{
                padding: "14px 20px 15px",
                borderTop: index === 0 ? "none" : "1px solid var(--border)",
              }}
            >
              <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: "14px" }}>
                <span style={{ minWidth: 0 }}>
                  <strong style={{ display: "block", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "13px", fontWeight: 650, color: "var(--foreground)" }}>
                    {skill.name}
                  </strong>
                  <span style={{ display: "block", marginTop: "3px", fontSize: "11px", color: "var(--muted-foreground)" }}>
                    {skill.source_name}
                  </span>
                </span>
                {skill.clawhub_version ? (
                  <span style={{ flexShrink: 0, fontFamily: "var(--font-mono)", fontSize: "11px", fontVariantNumeric: "tabular-nums", color: "var(--muted-foreground)" }}>
                    v{skill.clawhub_version}
                  </span>
                ) : null}
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: "6px", marginTop: "10px" }}>
                {installations.map((installation) => {
                  const isGlobal = installation.scope === "global";
                  const label = isGlobal
                    ? t("marketplace.targetGlobal")
                    : installation.project_name ?? t("marketplace.targetProjectFallback");
                  return (
                    <span
                      key={installation.instance_id}
                      style={{
                        maxWidth: "100%",
                        display: "inline-flex",
                        alignItems: "center",
                        gap: "5px",
                        padding: "4px 7px",
                        borderRadius: "6px",
                        border: "1px solid var(--border)",
                        color: "var(--muted-foreground)",
                        backgroundColor: "var(--secondary)",
                        fontSize: "10px",
                        fontWeight: 550,
                      }}
                    >
                      {isGlobal ? <Globe2 size={11} /> : <FolderKanban size={11} />}
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{label}</span>
                    </span>
                  );
                })}
              </div>
            </article>
          ))}
        </div>

        <footer style={{ display: "flex", justifyContent: "flex-end", gap: "10px", padding: "16px 20px 20px" }}>
          <button
            type="button"
            onClick={onClose}
            disabled={updating}
            style={{ height: "36px", padding: "0 14px", border: "1px solid var(--border)", borderRadius: "8px", color: "var(--foreground)", backgroundColor: "var(--secondary)", fontSize: "12px", fontWeight: 600, cursor: updating ? "not-allowed" : "pointer" }}
          >
            {t("common.cancel")}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={updating || candidates.length === 0}
            style={{ minWidth: "116px", height: "36px", display: "inline-flex", alignItems: "center", justifyContent: "center", gap: "7px", padding: "0 16px", border: 0, borderRadius: "8px", color: "var(--primary-foreground)", backgroundColor: "var(--primary)", fontSize: "12px", fontWeight: 650, cursor: updating || candidates.length === 0 ? "not-allowed" : "pointer", opacity: updating || candidates.length === 0 ? 0.6 : 1 }}
          >
            <RefreshCw size={14} style={{ animation: updating ? "spin 1s linear infinite" : "none" }} />
            {updating ? t("marketplace.updatingAll") : t("marketplace.updateDialogConfirm")}
          </button>
        </footer>
      </section>
    </div>
  );
}
