import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, RefreshCw, ShieldAlert } from "lucide-react";
import { PageHeader } from "@/components/ui/page-header";
import {
  ReleaseHealth,
  EvaluationRecord,
  EvaluationStatus,
  ReleaseImprovementSuggestion,
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
  const [contextualHealth, setContextualHealth] = useState<ReleaseHealth | null>(null);
  const [releaseId, setReleaseId] = useState("");
  const [code, setCode] = useState<StudioFeedbackCode>("completed");
  const [evidenceType, setEvidenceType] =
    useState<StudioEvidenceType>("command_result");
  const [evidence, setEvidence] = useState("");
  const [contextProjectId, setContextProjectId] = useState("");
  const [contextWorkScope, setContextWorkScope] = useState("");
  const [contextProviderId, setContextProviderId] = useState("");
  const [evaluationCaseId, setEvaluationCaseId] = useState("");
  const [evaluationStatus, setEvaluationStatus] = useState<EvaluationStatus>("passed");
  const [evaluationEvidence, setEvaluationEvidence] = useState("");
  const [evaluations, setEvaluations] = useState<EvaluationRecord[]>([]);
  const [suggestions, setSuggestions] = useState<ReleaseImprovementSuggestion[]>([]);
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
            project_id: contextProjectId || null,
            work_scope: contextWorkScope || null,
            provider_id: contextProviderId || null,
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

  const loadContextualHealth = () =>
    void (async () => {
      if (!releaseId) return;
      setBusy(true);
      setError(null);
      try {
        setContextualHealth(
          await invoke<ReleaseHealth>("get_contextual_release_health", {
            request: {
              release_id: releaseId,
              project_id: contextProjectId || null,
              work_scope: contextWorkScope || null,
              provider_id: contextProviderId || null,
            },
          }),
        );
      } catch (loadError) {
        setError(String(loadError));
      } finally {
        setBusy(false);
      }
    })();

  const loadEvaluations = (selectedReleaseId: string) =>
    void (async () => {
      if (!selectedReleaseId) return;
      try {
        setEvaluations(
          await invoke<EvaluationRecord[]>("list_release_evaluations", {
            releaseId: selectedReleaseId,
          }),
        );
      } catch (loadError) {
        setError(String(loadError));
      }
    })();

  const submitEvaluation = () =>
    void (async () => {
      setBusy(true);
      setError(null);
      try {
        await invoke("record_release_evaluation", {
          request: {
            release_id: releaseId,
            case_id: evaluationCaseId,
            status: evaluationStatus,
            evidence_type: evidenceType,
            evidence_summary: evaluationEvidence,
            project_id: contextProjectId || null,
            work_scope: contextWorkScope || null,
            provider_id: contextProviderId || null,
          },
        });
        setEvaluationEvidence("");
        await loadEvaluations(releaseId);
        await load();
      } catch (submitError) {
        setError(String(submitError));
      } finally {
        setBusy(false);
      }
    })();

  const loadSuggestions = (selectedReleaseId: string) =>
    void (async () => {
      if (!selectedReleaseId) return;
      try {
        setSuggestions(
          await invoke<ReleaseImprovementSuggestion[]>(
            "get_release_improvement_suggestions",
            { releaseId: selectedReleaseId },
          ),
        );
      } catch (loadError) {
        setError(String(loadError));
      }
    })();

  const releaseById = (id: string): SkillSetRelease | undefined =>
    catalog?.releases.find((release) => release.id === id);
  const evaluationCases = (releaseById(releaseId)?.member_snapshots ?? []).flatMap((snapshot) =>
    (snapshot.evaluation_cases ?? []).map((casePath) => ({
      id: `${snapshot.skill_id}::${casePath}`,
      label: `${snapshot.skill_id} · ${casePath}`,
    })),
  );
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
              onChange={(event) => { setReleaseId(event.target.value); setEvaluationCaseId(""); void loadEvaluations(event.target.value); }}
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
          <div className="mt-3 grid grid-cols-3 gap-2">
            <label className="text-xs font-medium">Project ID<input value={contextProjectId} onChange={(event) => setContextProjectId(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm" placeholder="optional" /></label>
            <label className="text-xs font-medium">Work scope<input value={contextWorkScope} onChange={(event) => setContextWorkScope(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm" placeholder="optional" /></label>
            <label className="text-xs font-medium">Provider ID<input value={contextProviderId} onChange={(event) => setContextProviderId(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm" placeholder="optional" /></label>
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
          <div className="mt-3 rounded-md border border-border p-3 text-xs">
            <div className="flex items-center justify-between gap-3">
              <div><strong>Contextual health</strong><p className="mt-1 text-muted-foreground">Filters the metrics above by the optional project, work scope, and provider values.</p></div>
              <button type="button" onClick={loadContextualHealth} disabled={busy || !releaseId} className="rounded border border-border px-2 py-1">Check context</button>
            </div>
            {contextualHealth && <p className="mt-3">{contextualHealth.status.replace(/_/g, " ")} · Evaluated: {contextualHealth.evaluated_count} · Activation runs: {contextualHealth.usage_count} · Success: {contextualHealth.verified_success_rate == null ? "—" : `${Math.round(contextualHealth.verified_success_rate * 100)}%`}</p>}
          </div>
          <div className="mt-4 rounded-md border border-border p-3 text-xs">
            <div className="flex items-center justify-between gap-3"><div><strong>Feedback-informed suggestions</strong><p className="mt-1 text-muted-foreground">Repeated, evidence-backed outcomes only. Suggestions never edit a release or binding.</p></div><button type="button" onClick={() => loadSuggestions(releaseId)} disabled={busy || !releaseId} className="rounded border border-border px-2 py-1">Generate</button></div>
            {suggestions.length > 0 && <div className="mt-3 space-y-2">{suggestions.map((suggestion) => <article key={suggestion.code} className="rounded bg-muted/50 p-2"><p className="font-medium">{suggestion.title} <span className="text-muted-foreground">({suggestion.occurrence_count})</span></p><p className="mt-1 text-muted-foreground">{suggestion.rationale}</p><p className="mt-1">Next: {suggestion.suggested_action}</p></article>)}</div>}
          </div>
          <div className="mt-5 border-t border-border pt-4">
            <h3 className="text-sm font-semibold">Run a frozen evaluation case</h3>
            <p className="mt-1 text-xs text-muted-foreground">Cases are copied from the contract when the release is frozen. Record a redacted assertion after running the case in the intended environment.</p>
            <label className="mt-3 block text-xs font-medium">Evaluation case
              <select value={evaluationCaseId} onChange={(event) => setEvaluationCaseId(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                <option value="">Select frozen case</option>
                {evaluationCases.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}
              </select>
            </label>
            {evaluationCases.length === 0 && <p className="mt-2 text-xs text-warning">This release has no frozen evaluation cases. Create a new release after adding a managed contract.</p>}
            <div className="mt-3 grid grid-cols-2 gap-2"><label className="text-xs font-medium">Result
              <select value={evaluationStatus} onChange={(event) => setEvaluationStatus(event.target.value as EvaluationStatus)} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                {['passed', 'failed', 'blocked'].map((item) => <option key={item}>{item}</option>)}
              </select>
            </label><label className="text-xs font-medium">Evidence type
              <select value={evidenceType} onChange={(event) => setEvidenceType(event.target.value as StudioEvidenceType)} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
                {['command_result', 'evaluation_assertion', 'human_confirmation'].map((item) => <option key={item}>{item}</option>)}
              </select>
            </label></div>
            <label className="mt-3 block text-xs font-medium">Redacted result summary
              <textarea value={evaluationEvidence} onChange={(event) => setEvaluationEvidence(event.target.value)} className="mt-1 min-h-20 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" placeholder="e.g. happy path passed; assertion matched expected output" />
            </label>
            <button type="button" onClick={submitEvaluation} disabled={busy || !releaseId || !evaluationCaseId || !evaluationEvidence.trim()} className="mt-3 rounded-md border border-primary/50 px-3 py-2 text-xs font-medium text-primary disabled:opacity-50">Record evaluation</button>
            <button type="button" onClick={() => loadEvaluations(releaseId)} disabled={busy || !releaseId} className="ml-2 rounded-md border border-border px-3 py-2 text-xs">View records</button>
            {evaluations.length > 0 && <div className="mt-3 rounded-md bg-muted/50 p-3 text-xs"><strong>Recent evaluation records</strong><div className="mt-2 space-y-1">{evaluations.map((record) => <p key={record.id}>{record.status} · {record.case_id} <span className="text-muted-foreground">· {new Date(record.created_at * 1000).toLocaleString()}</span></p>)}</div></div>}
          </div>
        </section>
      </div>
    </div>
  );
}
