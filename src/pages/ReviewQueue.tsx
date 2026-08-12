import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, RefreshCw, ShieldAlert } from "lucide-react";
import { PageHeader } from "@/components/ui/page-header";
import {
  ReleaseHealth,
  ReviewQueueItem,
  SkillSetRelease,
  SkillSetStore,
  StudioEvidenceType,
  StudioFeedbackCode,
} from "@/types";

export function ReviewQueue() {
  const [catalog, setCatalog] = useState<SkillSetStore | null>(null);
  const [queue, setQueue] = useState<ReviewQueueItem[]>([]);
  const [health, setHealth] = useState<Record<string, ReleaseHealth>>({});
  const [releaseId, setReleaseId] = useState("");
  const [code, setCode] = useState<StudioFeedbackCode>("completed");
  const [evidenceType, setEvidenceType] =
    useState<StudioEvidenceType>("command_result");
  const [evidence, setEvidence] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [nextCatalog, nextQueue] = await Promise.all([
        invoke<SkillSetStore>("get_skill_set_catalog"),
        invoke<ReviewQueueItem[]>("get_studio_review_queue"),
      ]);
      const healthEntries = await Promise.all(
        nextCatalog.releases.map(
          async (release) =>
            [
              release.id,
              await invoke<ReleaseHealth>("get_release_health", {
                releaseId: release.id,
              }),
            ] as const,
        ),
      );
      setCatalog(nextCatalog);
      setQueue(nextQueue);
      setHealth(Object.fromEntries(healthEntries));
      setReleaseId((current) => current || nextCatalog.releases[0]?.id || "");
    } catch (loadError) {
      setError(String(loadError));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const submit = () =>
    void (async () => {
      setBusy(true);
      setError(null);
      try {
        await invoke("record_studio_feedback", {
          request: {
            target_kind: "skill_set_release",
            target_id: releaseId,
            code,
            evidence_type: evidenceType,
            evidence_summary: evidence,
          },
        });
        setEvidence("");
        await load();
      } catch (submitError) {
        setError(String(submitError));
      } finally {
        setBusy(false);
      }
    })();

  const releaseById = (id: string): SkillSetRelease | undefined =>
    catalog?.releases.find((release) => release.id === id);
  return (
    <div className="h-full overflow-auto px-6 py-5">
      <PageHeader
        title="Review Queue"
        actions={
          <button
            type="button"
            onClick={() => void load()}
            className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-xs"
            disabled={busy}
          >
            <RefreshCw size={14} /> Refresh
          </button>
        }
      />
      <p className="mt-1 text-sm text-muted-foreground">
        Health stays unknown until it has at least five evidence-backed
        outcomes. Safety concerns are always shown separately.
      </p>
      {error && (
        <div className="mt-4 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </div>
      )}
      <div className="mt-5 grid gap-5 xl:grid-cols-[1.1fr_0.9fr]">
        <section className="rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2">
            <AlertTriangle size={16} />
            <h2 className="text-sm font-semibold">Items requiring attention</h2>
          </div>
          <div className="mt-3 space-y-2">
            {queue.map((item) => {
              const release = releaseById(item.release_id);
              return (
                <article
                  key={`${item.release_id}:${item.reason}`}
                  className="rounded-md border border-border p-3"
                >
                  <div className="flex items-start gap-2">
                    <ShieldAlert size={15} className="mt-0.5 text-warning" />
                    <div>
                      <h3 className="text-sm font-medium">
                        {release?.blueprint_name ?? item.release_id}
                      </h3>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {item.detail}
                      </p>
                      <p className="mt-2 text-[11px] uppercase tracking-wide text-muted-foreground">
                        {item.reason.replace(/_/g, " ")}
                      </p>
                    </div>
                  </div>
                </article>
              );
            })}
            {queue.length === 0 && (
              <p className="py-5 text-center text-sm text-muted-foreground">
                No review items. This does not hide future safety events.
              </p>
            )}
          </div>
        </section>
        <section className="rounded-lg border border-border bg-card p-4">
          <h2 className="text-sm font-semibold">Record evaluated feedback</h2>
          <p className="mt-1 text-xs text-muted-foreground">
            Store only a redacted evidence summary; raw output is not retained.
          </p>
          <label className="mt-4 block text-xs font-medium">
            Release
            <select
              value={releaseId}
              onChange={(event) => setReleaseId(event.target.value)}
              className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              {catalog?.releases.map((release) => (
                <option key={release.id} value={release.id}>
                  {release.blueprint_name} · {release.label}
                </option>
              ))}
            </select>
          </label>
          <div className="mt-3 grid grid-cols-2 gap-2">
            <label className="text-xs font-medium">
              Outcome
              <select
                value={code}
                onChange={(event) =>
                  setCode(event.target.value as StudioFeedbackCode)
                }
                className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                {[
                  "completed",
                  "partial",
                  "failed",
                  "wrong_scope",
                  "instruction_gap",
                  "dependency_gap",
                  "safety_concern",
                ].map((item) => (
                  <option key={item}>{item}</option>
                ))}
              </select>
            </label>
            <label className="text-xs font-medium">
              Evidence type
              <select
                value={evidenceType}
                onChange={(event) =>
                  setEvidenceType(event.target.value as StudioEvidenceType)
                }
                className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                {[
                  "command_result",
                  "evaluation_assertion",
                  "human_confirmation",
                ].map((item) => (
                  <option key={item}>{item}</option>
                ))}
              </select>
            </label>
          </div>
          <label className="mt-3 block text-xs font-medium">
            Redacted evidence summary
            <textarea
              value={evidence}
              onChange={(event) => setEvidence(event.target.value)}
              className="mt-1 min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              placeholder="e.g. cargo test passed; reviewer confirmed expected output"
            />
          </label>
          <button
            type="button"
            onClick={submit}
            disabled={busy || !releaseId || !evidence.trim()}
            className="mt-3 rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground disabled:opacity-50"
          >
            Record feedback
          </button>
          {releaseId && health[releaseId] && (
            <div className="mt-5 rounded-md bg-muted/50 p-3 text-xs">
              <strong>
                Current health: {health[releaseId].status.replace(/_/g, " ")}
              </strong>
              <p className="mt-1">
                Evaluated: {health[releaseId].evaluated_count} · Success:{" "}
                {health[releaseId].verified_success_rate == null
                  ? "—"
                  : `${Math.round(health[releaseId].verified_success_rate * 100)}%`}{" "}
                · Safety incidents: {health[releaseId].safety_incidents}
              </p>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
