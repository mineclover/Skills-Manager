import type { SkillOperationReport } from "@/types";

/** Combine per-skill reports into the same auditable shape returned by batch commands. */
export function mergeSkillOperationReports(
  reports: SkillOperationReport[],
): SkillOperationReport | null {
  if (reports.length === 0) {
    return null;
  }

  const first = reports[0];
  const same = <T,>(select: (report: SkillOperationReport) => T): T | null => {
    const value = select(first);
    return reports.every((report) => select(report) === value) ? value : null;
  };
  const impacts = Array.from(
    new Map(
      reports
        .flatMap((report) => report.impacts)
        .map((impact) => [`${impact.provider_id}:${impact.root_path ?? ""}`, impact]),
    ).values(),
  );

  return {
    operation_id: reports.map((report) => report.operation_id).join(","),
    action: first.action,
    scope: same((report) => report.scope),
    project_id: same((report) => report.project_id),
    provider_id: same((report) => report.provider_id),
    requested_count: reports.reduce((total, report) => total + report.requested_count, 0),
    attempted_count: reports.reduce((total, report) => total + report.attempted_count, 0),
    applied_count: reports.reduce((total, report) => total + report.applied_count, 0),
    skipped_count: reports.reduce((total, report) => total + report.skipped_count, 0),
    failed_count: reports.reduce((total, report) => total + report.failed_count, 0),
    failures: reports.flatMap((report) => report.failures),
    impacts,
    completed_at: Math.max(...reports.map((report) => report.completed_at)),
  };
}
