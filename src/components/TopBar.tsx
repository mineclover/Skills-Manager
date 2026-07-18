import { useEffect, useRef } from "react";
import { ScopeSearchField } from "@/components/ScopeSearchField";
import { useActionsTarget, usePageHeaderState } from "@/components/PageHeaderContext";
import { useTranslation } from "@/i18n";

interface TopBarProps {
  onOpenPalette: () => void;
}

export function TopBar({ onOpenPalette }: TopBarProps) {
  const actionsSlotRef = useRef<HTMLDivElement | null>(null);
  const { registerActionsTarget } = useActionsTarget();
  const { title } = usePageHeaderState();
  const { t } = useTranslation();

  // Register the actions slot as the portal target for PageHeader actions.
  useEffect(() => {
    registerActionsTarget?.(actionsSlotRef.current);
    return () => registerActionsTarget?.(null);
  }, [registerActionsTarget]);

  return (
    <header
      className="glass"
      data-tauri-drag-region
      style={{
        height: 52,
        minHeight: 52,
        display: "flex",
        alignItems: "center",
        padding: "0 16px",
        gap: 16,
        border: "none",
        borderBottom: "1px solid var(--glass-border)",
        position: "relative",
        zIndex: 50,
        cursor: "grab",
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          flex: 1,
          minWidth: 0,
        }}
      >
        <span style={{ color: "var(--ember)", fontSize: 13 }}>✦</span>
        <span
          style={{
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            color: "var(--foreground)",
            fontSize: 12,
            fontWeight: 650,
          }}
        >
          {title || t("topbar.brand")}
        </span>
      </div>

      {/* Center scope search — the field shows the current page as a chip */}
      <ScopeSearchField onOpenPalette={onOpenPalette} />

      {/* Page actions — portalled here by the active page's <PageHeader/> */}
      <div
        ref={actionsSlotRef}
        aria-label={title || t("topbar.brand")}
        style={{ display: "flex", alignItems: "center", gap: 8, flex: 1, justifyContent: "flex-end", minWidth: 0, minHeight: 28 }}
      />
    </header>
  );
}
