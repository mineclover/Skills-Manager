import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { PageHeader } from "@/components/ui/page-header";
import {
  AppConfig,
  EffectiveSkillSet,
  SkillSetAssignment,
  SkillSetRelease,
  SkillSetStore,
} from "@/types";

const emptyStore: SkillSetStore = {
  schema_version: 1,
  blueprints: [],
  releases: [],
  assignments: [],
};

function compareAssignments(left: SkillSetAssignment, right: SkillSetAssignment) {
  return right.priority - left.priority || left.created_at - right.created_at;
}

export function ProjectProfile() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [store, setStore] = useState<SkillSetStore>(emptyStore);
  const [projectId, setProjectId] = useState("");
  const [workScope, setWorkScope] = useState("");
  const [effectiveSet, setEffectiveSet] = useState<EffectiveSkillSet | null>(null);
  const [draftCreated, setDraftCreated] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const [nextConfig, nextStore] = await Promise.all([
        invoke<AppConfig>("get_config"),
        invoke<SkillSetStore>("get_skill_set_catalog"),
      ]);
      setConfig(nextConfig);
      setStore(nextStore);
      setProjectId((current) => current || nextConfig.active_project_id || nextConfig.projects?.[0]?.id || "");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const projectAssignments = useMemo(
    () => store.assignments
      .filter((assignment) => !assignment.project_id || assignment.project_id === projectId)
      .sort(compareAssignments),
    [projectId, store.assignments],
  );
  const releases = useMemo(
    () => new Map(store.releases.map((release) => [release.id, release])),
    [store.releases],
  );
  const scopes = useMemo(
    () => [...new Set(projectAssignments
      .filter((assignment) => assignment.role === "recommended")
      .map((assignment) => assignment.work_scope))],
    [projectAssignments],
  );

  const resolve = () => void (async () => {
    if (!workScope.trim()) return;
    setBusy(true);
    setError(null);
    try {
      setEffectiveSet(await invoke<EffectiveSkillSet>("resolve_effective_skill_set", {
        request: { project_id: projectId || null, work_scope: workScope },
      }));
    } catch (resolveError) {
      setError(String(resolveError));
    } finally {
      setBusy(false);
    }
  })();

  const createContextDraft = () => void (async () => {
    if (!effectiveSet) return;
    setBusy(true);
    setError(null);
    try {
      const projectName = (config?.projects ?? []).find((project) => project.id === projectId)?.name ?? "Global";
      const catalog = await invoke<SkillSetStore>("create_skill_set_blueprint", {
        request: {
          name: `${projectName} ${effectiveSet.work_scope} draft`,
          description: `Drafted from the ${projectName} project profile for ${effectiveSet.work_scope}. Review the purpose, requirements, and members before releasing.`,
          skill_ids: effectiveSet.members.map((member) => member.skill_id),
          member_scope_policies: Object.fromEntries(effectiveSet.members.map((member) => [member.skill_id, member.scope_policy])),
        },
      });
      setStore(catalog);
      setDraftCreated("Editable blueprint created. It remains unreleased until a human reviews it.");
    } catch (draftError) {
      setError(String(draftError));
    } finally {
      setBusy(false);
    }
  })();

  const changePriority = (assignment: SkillSetAssignment, delta: number) =>
    void (async () => {
      setBusy(true);
      setError(null);
      try {
        setStore(await invoke<SkillSetStore>("set_skill_set_assignment_priority", {
          request: { assignment_id: assignment.id, priority: assignment.priority + delta },
        }));
      } catch (changeError) {
        setError(String(changeError));
      } finally {
        setBusy(false);
      }
    })();

  const releaseLabel = (release?: SkillSetRelease) =>
    release ? `${release.blueprint_name}${release.label ? ` · ${release.label}` : ""}` : "Missing release";

  return (
    <div className="h-full overflow-auto px-6 py-5">
      <PageHeader
        title="Project Profile"
        actions={<button type="button" onClick={() => void load()} disabled={busy} className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-xs"><RefreshCw size={14} /> Refresh</button>}
      />
      <p className="mt-1 text-sm text-muted-foreground">Choose a project and current work scope before changing provider bindings. Priority controls the explanation order when several assignments apply.</p>
      {error && <div className="mt-4 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</div>}
      <section className="mt-5 rounded-lg border border-border bg-card p-4">
        <div className="grid gap-3 md:grid-cols-[1fr_1fr_auto]">
          <label className="text-xs font-medium">Project<select value={projectId} onChange={(event) => { setProjectId(event.target.value); setEffectiveSet(null); }} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"><option value="">Global / no project</option>{(config?.projects ?? []).map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
          <label className="text-xs font-medium">Work scope<input list="project-work-scopes" value={workScope} onChange={(event) => setWorkScope(event.target.value)} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" placeholder="upstream-integration" /><datalist id="project-work-scopes">{scopes.map((scope) => <option key={scope} value={scope} />)}</datalist></label>
          <button type="button" onClick={resolve} disabled={busy || !workScope.trim()} className="self-end rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground disabled:opacity-50">Resolve effective set</button>
        </div>
      </section>
      <div className="mt-5 grid gap-5 xl:grid-cols-[1fr_1fr]">
        <section className="rounded-lg border border-border bg-card p-4">
          <h2 className="text-sm font-semibold">Default and recommended releases</h2>
          <p className="mt-1 text-xs text-muted-foreground">Higher priority is considered first. Project assignments and global baselines are both visible.</p>
          <div className="mt-3 space-y-2">
            {projectAssignments.map((assignment) => { const release = releases.get(assignment.release_id); const missingContracts = release?.member_snapshots.filter((member) => member.contract_status !== "managed") ?? []; return <article key={assignment.id} className="rounded-md border border-border p-3 text-xs"><div className="flex items-start gap-3"><div className="min-w-0 flex-1"><strong>{releaseLabel(release)}</strong><p className="mt-1 text-muted-foreground">{assignment.project_id ? "Project" : "Global baseline"} · {assignment.role === "default" ? "Default for every work scope" : `Recommended for ${assignment.work_scope}`} · {assignment.active ? "Active" : "Inactive"}</p><p className="mt-1 text-muted-foreground">Providers: {assignment.provider_ids.join(", ") || "Not selected"}</p>{missingContracts.length > 0 && <p className="mt-1 text-warning">Missing managed contract: {missingContracts.map((member) => member.skill_id).join(", ")}</p>}</div><div className="flex items-center gap-1"><span className="mr-1 rounded bg-muted px-2 py-1">P{assignment.priority}</span><button type="button" onClick={() => changePriority(assignment, 1)} disabled={busy} className="rounded border border-border px-2 py-1">↑</button><button type="button" onClick={() => changePriority(assignment, -1)} disabled={busy} className="rounded border border-border px-2 py-1">↓</button></div></div></article>; })}
            {projectAssignments.length === 0 && <p className="py-5 text-center text-sm text-muted-foreground">No default or recommended releases are assigned to this project yet.</p>}
          </div>
        </section>
        <section className="rounded-lg border border-border bg-card p-4">
          <h2 className="text-sm font-semibold">Resolved configuration</h2>
          {!effectiveSet && <p className="mt-3 text-sm text-muted-foreground">Select a work scope to see the exact skill instances that would be used. This step is read-only.</p>}
          {effectiveSet && <div className="mt-3 text-xs"><div className="flex items-center justify-between gap-3"><p><strong>{effectiveSet.release_ids.length} releases · {effectiveSet.members.length} skills</strong></p><button type="button" onClick={createContextDraft} disabled={busy || effectiveSet.members.length === 0} className="rounded border border-border px-2 py-1">Create editable draft</button></div>{draftCreated && <p className="mt-2 text-primary">{draftCreated}</p>}{effectiveSet.unresolved_skill_ids.length > 0 && <p className="mt-2 text-destructive">Preview blocked — missing required instance: {effectiveSet.unresolved_skill_ids.join(", ")}</p>}<div className="mt-3 space-y-2">{effectiveSet.members.map((member) => <div key={`${member.skill_id}:${member.scope_policy}`} className="rounded bg-muted/50 p-2"><strong>{member.skill_id}</strong><p className="mt-1 text-muted-foreground">{member.scope_policy.replace(/_/g, " ")} · {member.skill_instance_id ?? "unresolved"}</p></div>)}</div></div>}
        </section>
      </div>
    </div>
  );
}
