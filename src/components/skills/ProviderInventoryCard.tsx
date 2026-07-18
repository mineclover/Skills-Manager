import { useState } from "react";
import { ChevronDown, ChevronRight, CircleAlert, CircleCheck, RadioTower } from "lucide-react";

import { SkillProviderInventory } from "@/types";
import { useTranslation } from "@/i18n";

interface ProviderInventoryCardProps {
  inventory: SkillProviderInventory;
}

function formatProviderKind(kind: string): string {
  return kind.replace(/_/g, " ");
}

function formatBoolean(value: boolean | null | undefined, unknownLabel: string): string {
  if (value === undefined || value === null) {
    return unknownLabel;
  }
  return value ? "true" : "false";
}

export function ProviderInventoryCard({ inventory }: ProviderInventoryCardProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const availableProviders = inventory.providers.filter((provider) => provider.detected).length;

  return (
    <section
      style={{
        marginBottom: "16px",
        border: "1px solid var(--border)",
        borderRadius: "12px",
        background: "var(--card)",
        overflow: "hidden",
      }}
    >
      <button
        type="button"
        onClick={() => setExpanded((current) => !current)}
        aria-expanded={expanded}
        style={{
          width: "100%",
          display: "flex",
          alignItems: "center",
          gap: "10px",
          padding: "12px 14px",
          color: "var(--foreground)",
          background: "transparent",
          border: 0,
          cursor: "pointer",
          textAlign: "left",
        }}
      >
        {expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
        <RadioTower size={16} style={{ color: "var(--primary)" }} />
        <span style={{ flex: 1, minWidth: 0 }}>
          <span style={{ display: "block", fontSize: "13px", fontWeight: 700 }}>
            {t("skills.providerInventoryTitle")}
          </span>
          <span style={{ display: "block", marginTop: "2px", fontSize: "11px", color: "var(--muted-foreground)" }}>
            {t("skills.providerInventorySummary")
              .replace("{available}", String(availableProviders))
              .replace("{total}", String(inventory.providers.length))
              .replace("{topics}", String(inventory.orca.topics.length))}
          </span>
        </span>
        {inventory.orca.available ? (
          <CircleCheck size={16} style={{ color: "var(--success, #22c55e)" }} />
        ) : (
          <CircleAlert size={16} style={{ color: "var(--warning, #f59e0b)" }} />
        )}
      </button>

      {expanded && (
        <div style={{ padding: "0 14px 14px", display: "flex", flexDirection: "column", gap: "8px" }}>
          {inventory.providers.map((provider) => (
            <div
              key={provider.provider_id}
              style={{
                display: "grid",
                gridTemplateColumns: "minmax(150px, 1fr) auto auto auto",
                alignItems: "center",
                gap: "10px",
                padding: "9px 10px",
                borderRadius: "8px",
                background: "var(--secondary)",
                fontSize: "11px",
              }}
            >
              <div style={{ minWidth: 0 }}>
                <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: 650, color: "var(--foreground)" }}>
                  {provider.detected ? (
                    <CircleCheck size={13} style={{ color: "var(--success, #22c55e)", flexShrink: 0 }} />
                  ) : (
                    <CircleAlert size={13} style={{ color: "var(--muted-foreground)", flexShrink: 0 }} />
                  )}
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {provider.display_name}
                  </span>
                </div>
                <div style={{ marginTop: "3px", color: "var(--muted-foreground)", textTransform: "capitalize" }}>
                  {provider.provider_id} · {formatProviderKind(provider.kind)}
                </div>
              </div>
              <span title={t("skills.providerSkillsCount")} style={{ color: "var(--muted-foreground)", whiteSpace: "nowrap" }}>
                {provider.skill_count} {t("skills.providerSkillsShort")}
              </span>
              <span style={{ color: provider.enabled_count > 0 ? "var(--primary)" : "var(--muted-foreground)", whiteSpace: "nowrap" }}>
                {provider.enabled_count}/{provider.skill_count} {t("skills.providerEnabledShort")}
              </span>
              <span style={{ color: "var(--muted-foreground)", whiteSpace: "nowrap" }}>
                {formatBoolean(provider.reachable, t("skills.providerUnknown"))}
              </span>
            </div>
          ))}

          <div style={{ padding: "10px", borderRadius: "8px", border: "1px solid var(--border)", fontSize: "11px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "7px", fontWeight: 650, color: "var(--foreground)" }}>
              {inventory.orca.available ? (
                <CircleCheck size={13} style={{ color: "var(--success, #22c55e)" }} />
              ) : (
                <CircleAlert size={13} style={{ color: "var(--warning, #f59e0b)" }} />
              )}
              <span>{t("skills.orcaInventory")}</span>
            </div>
            <div style={{ marginTop: "5px", color: "var(--muted-foreground)", lineHeight: 1.5 }}>
              {t("skills.orcaInventoryStatus")
                .replace("{cli}", formatBoolean(inventory.orca.cli_available, t("skills.providerUnknown")))
                .replace("{reachable}", formatBoolean(inventory.orca.runtime_reachable, t("skills.providerUnknown")))
                .replace("{topics}", String(inventory.orca.topics.length))}
            </div>
            {inventory.orca.warning && (
              <div style={{ marginTop: "5px", color: "var(--warning, #f59e0b)" }}>
                {inventory.orca.warning}
              </div>
            )}
            {inventory.orca.topics.length > 0 && (
              <div style={{ display: "flex", flexWrap: "wrap", gap: "5px", marginTop: "7px" }}>
                {inventory.orca.topics.map((topic) => (
                  <span
                    key={topic.name}
                    title={topic.description ?? topic.name}
                    style={{ padding: "3px 6px", borderRadius: "999px", color: "var(--primary)", background: "var(--primary-tint)" }}
                  >
                    {topic.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
