import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "@/i18n";
import { CliInstallStatus } from "@/types";
import {
  MODAL_LAYER_Z_INDEX,
  MODAL_OVERLAY_COLOR,
} from "@/constants/modal";

// Show the CLI promo at most this many times across app launches.
const MAX_PROMO_COUNT = 3;
const PROMO_COUNT_STORAGE_KEY = "skills-manager:cli-promo-shown-count";

function readPromoCount(): number {
  try {
    const raw = window.localStorage.getItem(PROMO_COUNT_STORAGE_KEY);
    const parsed = raw ? parseInt(raw, 10) : 0;
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  } catch {
    return 0;
  }
}

function writePromoCount(count: number): void {
  try {
    window.localStorage.setItem(PROMO_COUNT_STORAGE_KEY, String(count));
  } catch {
    // ignore storage errors
  }
}

export function CliPromoModal() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [status, setStatus] = useState<CliInstallStatus | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function check() {
      // Already shown enough times — never show again.
      if (readPromoCount() >= MAX_PROMO_COUNT) return;
      try {
        const cliStatus = await invoke<CliInstallStatus>("get_cli_install_status");
        if (cancelled) return;
        // Skip when the binary is missing (dev build) or already installed and current.
        if (!cliStatus.bundled || (cliStatus.installed && cliStatus.versionMatches)) return;
        // Count actual displays so closing the app without clicking still uses up a slot.
        writePromoCount(readPromoCount() + 1);
        setStatus(cliStatus);
        setVisible(true);
      } catch {
        // Resource missing or command failed — stay silent.
      }
    }

    void check();
    return () => {
      cancelled = true;
    };
  }, []);

  const dismiss = useCallback(() => {
    setVisible(false);
  }, []);

  // Escape closes it, same as the marketplace dialogs. This modal appears
  // unprompted at launch, so a keyboard user needs a way out that is not "hunt
  // for the Later button".
  useEffect(() => {
    if (!visible) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [visible, dismiss]);

  const goSettings = useCallback(() => {
    setVisible(false);
    navigate("/settings");
  }, [navigate]);

  if (!visible || !status) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        backgroundColor: MODAL_OVERLAY_COLOR,
        zIndex: MODAL_LAYER_Z_INDEX,
        padding: "24px",
      }}
      onClick={dismiss}
    >
      <div
        className="animate-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cli-promo-title"
        style={{
          width: "min(420px, calc(100vw - 48px))",
          background: "var(--background)",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius-xl)",
          boxShadow: "0 18px 60px rgba(0,0,0,0.25)",
          overflow: "hidden",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div style={{ padding: "22px 22px 0" }}>
          <h3
            id="cli-promo-title"
            style={{
              margin: 0,
              fontSize: "16px",
              fontWeight: 600,
              color: "var(--foreground)",
              letterSpacing: "-0.01em",
            }}
          >
            {t("cli.promoTitle")}
          </h3>
          <p
            style={{
              margin: "10px 0 0 0",
              fontSize: "13px",
              color: "var(--muted-foreground)",
              lineHeight: 1.6,
            }}
          >
            {t("cli.promoDescription")}
          </p>
        </div>

        {/* Footer actions */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "flex-end",
            gap: "10px",
            padding: "20px 22px 22px",
          }}
        >
          <button
            type="button"
            onClick={dismiss}
            style={{
              padding: "8px 14px",
              fontSize: "13px",
              fontWeight: 500,
              color: "var(--muted-foreground)",
              backgroundColor: "transparent",
              border: "none",
              borderRadius: "8px",
              cursor: "pointer",
              transition: "color 0.15s, background-color 0.15s",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.color = "var(--foreground)";
              e.currentTarget.style.backgroundColor = "var(--muted)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.color = "var(--muted-foreground)";
              e.currentTarget.style.backgroundColor = "transparent";
            }}
          >
            {t("cli.promoLater")}
          </button>
          <button
            type="button"
            onClick={goSettings}
            autoFocus
            style={{
              padding: "8px 16px",
              fontSize: "13px",
              fontWeight: 500,
              color: "var(--primary-foreground)",
              backgroundColor: "var(--primary)",
              border: "none",
              borderRadius: "8px",
              cursor: "pointer",
              transition: "opacity 0.15s",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.opacity = "0.9")}
            onMouseLeave={(e) => (e.currentTarget.style.opacity = "1")}
          >
            {t("cli.promoGoSettings")}
          </button>
        </div>
      </div>
    </div>
  );
}
