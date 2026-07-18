import { AlertTriangle, CheckCircle2, RadioTower } from "lucide-react";

import { SkillOperationReport } from "@/types";
import { useTranslation } from "@/i18n";

interface OperationReportCardProps {
  report: SkillOperationReport;
  scopeLabel: string;
  providerLabel: string;
}

export function OperationReportCard({ report, scopeLabel, providerLabel }: OperationReportCardProps) {
  const { t } = useTranslation();
  const hasFailures = report.failed_count > 0;
  const hasImpacts = report.impacts.length > 0;

  return (
    <section
      role={hasFailures ? "alert" : "status"}
      className={`mx-6 mt-4 rounded-lg border px-4 py-3 ${
        hasFailures
          ? "border-destructive/30 bg-destructive/5"
          : "border-emerald-600/25 bg-emerald-600/5"
      }`}
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-2">
          {hasFailures ? (
            <AlertTriangle size={15} className="mt-0.5 shrink-0 text-amber-600" />
          ) : (
            <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-emerald-600" />
          )}
          <div className="min-w-0">
            <div className="text-xs font-semibold text-foreground">{t("presets.operationReport")}</div>
            <div className="mt-0.5 truncate text-[10px] text-muted-foreground" title={`${scopeLabel} · ${providerLabel}`}>
              {scopeLabel} · {providerLabel}
            </div>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-muted-foreground tabular-nums">
          <span>{t("presets.reportApplied").replace("{count}", String(report.applied_count))}</span>
          <span>{t("presets.reportSkipped").replace("{count}", String(report.skipped_count))}</span>
          <span className={hasFailures ? "font-semibold text-destructive" : ""}>
            {t("presets.reportFailed").replace("{count}", String(report.failed_count))}
          </span>
        </div>
      </div>

      {hasImpacts && (
        <div className="mt-2 flex items-start gap-2 border-t border-border/70 pt-2 text-[10px] text-muted-foreground">
          <RadioTower size={13} className="mt-0.5 shrink-0" />
          <span>
            {t("presets.reportImpacts").replace(
              "{providers}",
              report.impacts.map((impact) => impact.display_name).join(", "),
            )}
          </span>
        </div>
      )}

      {hasFailures && (
        <ul className="mt-2 space-y-1 border-t border-border/70 pt-2 text-[10px] text-destructive">
          {report.failures.slice(0, 3).map((failure, index) => (
            <li key={`${failure.provider_id ?? "failure"}-${index}`}>{failure.message}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
