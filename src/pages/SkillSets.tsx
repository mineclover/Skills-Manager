import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Layers3, Plus, RefreshCw, Trash2 } from "lucide-react";
import { PageHeader } from "@/components/ui/page-header";
import {
  AppConfig,
  Skill,
  SkillSetAssignment,
  SkillSetActivationPlan,
  SkillSetActivationApplyResult,
  SkillSetBlueprint,
  SkillSetRelease,
  SkillSetStore,
} from "@/types";

const emptyStore: SkillSetStore = {
  schema_version: 1,
  blueprints: [],
  releases: [],
  assignments: [],
};

export function SkillSets() {
  const [store, setStore] = useState<SkillSetStore>(emptyStore);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [memberIds, setMemberIds] = useState<Set<string>>(new Set());
  const [assignmentScope, setAssignmentScope] = useState("");
  const [assignmentProjectId, setAssignmentProjectId] = useState("");
  const [providerIds, setProviderIds] = useState("");
  const [plan, setPlan] = useState<SkillSetActivationPlan | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [catalog, listedSkills, appConfig] = await Promise.all([
        invoke<SkillSetStore>("get_skill_set_catalog"),
        invoke<Skill[]>("list_skills"),
        invoke<AppConfig>("get_config"),
      ]);
      setStore(catalog);
      setSkills(listedSkills);
      setConfig(appConfig);
    } catch (loadError) {
      setError(String(loadError));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const skillsById = useMemo(() => {
    const map = new Map<string, Skill>();
    for (const skill of skills) {
      if (!map.has(skill.id)) map.set(skill.id, skill);
    }
    return map;
  }, [skills]);

  const updateStore = async (operation: () => Promise<SkillSetStore>) => {
    setBusy(true);
    setError(null);
    try {
      setStore(await operation());
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  };

  const createBlueprint = () => void updateStore(async () => {
    const catalog = await invoke<SkillSetStore>("create_skill_set_blueprint", {
      request: { name, description, skill_ids: [...memberIds] },
    });
    setName("");
    setDescription("");
    setMemberIds(new Set());
    return catalog;
  });

  const createRelease = (blueprint: SkillSetBlueprint) => void updateStore(() =>
    invoke<SkillSetStore>("create_skill_set_release", {
      request: { blueprint_id: blueprint.id, label: `snapshot-${new Date().toISOString().slice(0, 10)}` },
    }),
  );

  const assignRelease = (release: SkillSetRelease) => void updateStore(() =>
    invoke<SkillSetStore>("assign_skill_set_release", {
      request: {
        release_id: release.id,
        project_id: assignmentProjectId || null,
        work_scope: assignmentScope,
        provider_ids: providerIds.split(",").map((value) => value.trim()).filter(Boolean),
      },
    }),
  );

  const assignmentRelease = (assignment: SkillSetAssignment) =>
    store.releases.find((release) => release.id === assignment.release_id);

  const previewActivation = (assignmentId: string) => void (async () => {
    setBusy(true); setError(null);
    try { setPlan(await invoke<SkillSetActivationPlan>("preview_skill_set_activation", { assignmentId })); }
    catch (previewError) { setError(String(previewError)); }
    finally { setBusy(false); }
  })();

  const applyActivation = (assignmentId: string) => void (async () => {
    setBusy(true); setError(null);
    try { const result = await invoke<SkillSetActivationApplyResult>("apply_skill_set_activation", { assignmentId }); setPlan(result.plan); await load(); }
    catch (applyError) { setError(String(applyError)); }
    finally { setBusy(false); }
  })();

  return (
    <div className="h-full overflow-auto px-6 py-5">
      <PageHeader
        title="Skill Sets"
        actions={
          <button type="button" onClick={() => void load()} className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-xs" disabled={busy}>
            <RefreshCw size={14} /> Refresh
          </button>
        }
      />

      {error && <div className="mt-4 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</div>}

      <section className="mt-5 grid gap-5 xl:grid-cols-[minmax(320px,0.9fr)_minmax(380px,1.1fr)]">
        <div className="rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2"><Plus size={16} /><h2 className="text-sm font-semibold">New blueprint</h2></div>
          <p className="mt-1 text-xs text-muted-foreground">Membership uses canonical skill IDs; provider bindings are chosen later by activation control.</p>
          <label className="mt-4 block text-xs font-medium">Name
            <input value={name} onChange={(event) => setName(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" placeholder="Upstream integration" />
          </label>
          <label className="mt-3 block text-xs font-medium">Purpose
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} className="mt-1 min-h-20 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" placeholder="When this skill set should be used" />
          </label>
          <div className="mt-3">
            <span className="text-xs font-medium">Members</span>
            <div className="mt-1 max-h-64 overflow-auto rounded-md border border-border p-2">
              {[...skillsById.values()].map((skill) => (
                <label key={skill.id} className="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-muted">
                  <input type="checkbox" checked={memberIds.has(skill.id)} onChange={() => setMemberIds((current) => {
                    const next = new Set(current);
                    if (next.has(skill.id)) next.delete(skill.id); else next.add(skill.id);
                    return next;
                  })} />
                  <span className="min-w-0 truncate">{skill.name}</span><code className="ml-auto text-[10px] text-muted-foreground">{skill.id}</code>
                </label>
              ))}
              {skillsById.size === 0 && <p className="p-2 text-xs text-muted-foreground">No discovered skills yet.</p>}
            </div>
          </div>
          <button type="button" onClick={createBlueprint} disabled={busy || !name.trim() || memberIds.size === 0} className="mt-4 inline-flex items-center gap-2 rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground disabled:opacity-50"><Plus size={14} /> Create blueprint</button>
        </div>

        <div className="space-y-4">
          <div className="rounded-lg border border-border bg-card p-4">
            <div className="flex items-center gap-2"><Layers3 size={16} /><h2 className="text-sm font-semibold">Blueprints</h2></div>
            <div className="mt-3 space-y-2">
              {store.blueprints.map((blueprint) => (
                <article key={blueprint.id} className="rounded-md border border-border p-3">
                  <div className="flex gap-3"><div className="min-w-0 flex-1"><h3 className="text-sm font-medium">{blueprint.name}</h3><p className="mt-1 text-xs text-muted-foreground">{blueprint.description || "No purpose recorded."}</p><p className="mt-2 text-xs text-muted-foreground">{blueprint.members.map((member) => member.skill_id).join(", ")}</p></div>
                    <div className="flex shrink-0 items-start gap-2">{blueprint.reviewed_at ? <button type="button" onClick={() => createRelease(blueprint)} disabled={busy} className="rounded border border-border px-2 py-1 text-xs">Freeze release</button> : <button type="button" onClick={() => void updateStore(() => invoke<SkillSetStore>("review_skill_set_blueprint", { request: { blueprint_id: blueprint.id } }))} disabled={busy} className="rounded border border-primary/50 px-2 py-1 text-xs text-primary">Mark reviewed</button>}<button type="button" onClick={() => void updateStore(() => invoke<SkillSetStore>("delete_skill_set_blueprint", { blueprintId: blueprint.id }))} disabled={busy} className="rounded border border-border p-1 text-muted-foreground"><Trash2 size={14} /></button></div>
                  </div>
                </article>
              ))}
              {store.blueprints.length === 0 && <p className="py-4 text-center text-sm text-muted-foreground">No blueprints yet.</p>}
            </div>
          </div>

          <div className="rounded-lg border border-border bg-card p-4">
            <h2 className="text-sm font-semibold">Frozen releases and assignments</h2>
            <div className="mt-3 grid gap-2 sm:grid-cols-3"><label className="text-xs font-medium">Project<select value={assignmentProjectId} onChange={(event) => setAssignmentProjectId(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm"><option value="">Global / no project</option>{(config?.projects ?? []).map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label><label className="text-xs font-medium">Work scope<input value={assignmentScope} onChange={(event) => setAssignmentScope(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm" placeholder="upstream-integration" /></label><label className="text-xs font-medium">Tool providers<input value={providerIds} onChange={(event) => setProviderIds(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-2 py-2 text-sm" placeholder="codex, claude-code" /></label></div>
            <div className="mt-3 space-y-2">
              {store.releases.map((release) => (
                <article key={release.id} className="rounded-md border border-border p-3"><div className="flex flex-wrap items-start justify-between gap-2"><div><h3 className="text-sm font-medium">{release.blueprint_name} <span className="text-muted-foreground">{release.label}</span></h3><p className="mt-1 text-xs text-muted-foreground">{release.members.length} members · digest {release.content_digest.slice(0, 12)}</p>{release.member_snapshots.length > 0 && <p className="mt-1 text-[11px] text-muted-foreground">{release.member_snapshots.filter((item) => item.contract_status === "managed").length}/{release.member_snapshots.length} managed contracts frozen</p>}</div><button type="button" onClick={() => assignRelease(release)} disabled={busy || !assignmentScope.trim() || !providerIds.trim()} className="rounded border border-border px-2 py-1 text-xs">Assign</button></div></article>
              ))}
              {store.releases.length === 0 && <p className="py-2 text-sm text-muted-foreground">Freeze a blueprint to create an immutable release.</p>}
            </div>
            {store.assignments.length > 0 && <div className="mt-4 border-t border-border pt-3"><h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Assignments</h3><div className="mt-2 space-y-2">{store.assignments.map((assignment) => { const release = assignmentRelease(assignment); const project = (config?.projects ?? []).find((item) => item.id === assignment.project_id); return <div key={assignment.id} className="flex items-center gap-3 rounded-md bg-muted/40 px-3 py-2 text-xs"><span className="min-w-0 flex-1 truncate">{release?.blueprint_name ?? "Unknown release"} → {project?.name ?? "Global"} / {assignment.work_scope}</span><button type="button" onClick={() => previewActivation(assignment.id)} disabled={busy || !assignment.active} className="rounded border border-border px-2 py-1">Preview</button><button type="button" onClick={() => applyActivation(assignment.id)} disabled={busy || !assignment.active} className="rounded border border-primary/50 px-2 py-1 text-primary">Apply</button><button type="button" onClick={() => void updateStore(() => invoke<SkillSetStore>("set_skill_set_assignment_active", { request: { assignment_id: assignment.id, active: !assignment.active } }))} className="rounded border border-border px-2 py-1">{assignment.active ? "Active" : "Inactive"}</button><button type="button" onClick={() => void updateStore(() => invoke<SkillSetStore>("delete_skill_set_assignment", { assignmentId: assignment.id }))} className="text-muted-foreground"><Trash2 size={14} /></button></div>; })}</div></div>}
            {plan && <div className="mt-4 rounded-md border border-border bg-muted/30 p-3 text-xs"><div className="flex justify-between"><strong>Activation preview: {plan.work_scope}</strong><span>{plan.operations.filter((item) => item.action === "enable").length} enable · {plan.operations.filter((item) => item.action === "unchanged").length} unchanged</span></div>{plan.missing_skill_ids.length > 0 && <p className="mt-2 text-destructive">Missing: {plan.missing_skill_ids.join(", ")}</p>}<div className="mt-2 space-y-1">{plan.operations.map((operation) => <p key={`${operation.skill_instance_id}:${operation.tool_id}`}>{operation.action} {operation.skill_id} → {operation.tool_id} <span className="text-muted-foreground">({operation.reason})</span></p>)}</div></div>}
          </div>
        </div>
      </section>
    </div>
  );
}
