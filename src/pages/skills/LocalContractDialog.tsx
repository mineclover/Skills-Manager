import { useEffect, useState } from "react";
import type { Skill, SkillContract } from "@/types";

interface LocalContractDialogProps {
  skill: Skill | null;
  saving: boolean;
  onClose: () => void;
  onSave: (contract: SkillContract) => void;
}

const lines = (value: string[]) => value.join("\n");
const parseLines = (value: string) => value.split("\n").map((item) => item.trim()).filter(Boolean);

function draftFor(skill: Skill): SkillContract {
  return {
    schema_version: 1,
    purpose: { summary: skill.description?.trim() || `${skill.name} workflow`, use_when: [], avoid_when: [] },
    requirements: { runtimes: [], project_signals: [], verification: [] },
    success_contract: { expected_outcomes: [], non_goals: [], safety_rules: [] },
    feedback: { codes: ["completed", "partial", "failed"], required_for_completed: [] },
    evaluation: { cases: [], review_cycle_days: 90 },
  };
}

export function LocalContractDialog({ skill, saving, onClose, onSave }: LocalContractDialogProps) {
  const [contract, setContract] = useState<SkillContract | null>(null);

  useEffect(() => {
    setContract(skill ? (skill.contract.contract ?? draftFor(skill)) : null);
  }, [skill]);

  if (!skill || !contract) return null;
  const update = <K extends keyof SkillContract>(key: K, value: SkillContract[K]) => setContract((current) => current ? { ...current, [key]: value } : current);
  const fields: Array<[string, string, string[], (value: string[]) => void]> = [
    ["Use when", "One intended use per line", contract.purpose.use_when, (value) => update("purpose", { ...contract.purpose, use_when: value })],
    ["Avoid when", "One boundary per line", contract.purpose.avoid_when, (value) => update("purpose", { ...contract.purpose, avoid_when: value })],
    ["Verification", "One check per line", contract.requirements.verification, (value) => update("requirements", { ...contract.requirements, verification: value })],
    ["Expected outcomes", "One expected result per line", contract.success_contract.expected_outcomes, (value) => update("success_contract", { ...contract.success_contract, expected_outcomes: value })],
    ["Non-goals", "One boundary per line", contract.success_contract.non_goals, (value) => update("success_contract", { ...contract.success_contract, non_goals: value })],
    ["Safety rules", "One safety rule per line", contract.success_contract.safety_rules, (value) => update("success_contract", { ...contract.success_contract, safety_rules: value })],
    ["Completed evidence", "One required evidence item per line", contract.feedback.required_for_completed, (value) => update("feedback", { ...contract.feedback, required_for_completed: value })],
    ["Evaluation cases", "One case identifier per line", contract.evaluation.cases, (value) => update("evaluation", { ...contract.evaluation, cases: value })],
  ];

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-5" onClick={() => !saving && onClose()}>
      <section className="max-h-[90vh] w-full max-w-2xl overflow-auto rounded-lg border border-border bg-card p-5 shadow-xl" onClick={(event) => event.stopPropagation()}>
        <h2 className="text-base font-semibold">Local contract metadata</h2>
        <p className="mt-1 text-xs text-muted-foreground">Stored only in Skills Manager for {skill.instance_id}. A portable <code>skill-manager.yaml</code> always takes precedence when present.</p>
        <label className="mt-4 block text-xs font-medium">Purpose summary<input value={contract.purpose.summary} onChange={(event) => update("purpose", { ...contract.purpose, summary: event.target.value })} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" /></label>
        <div className="mt-3 grid gap-3 sm:grid-cols-2">
          {fields.map(([label, placeholder, value, setValue]) => <label key={label} className="text-xs font-medium">{label}<textarea value={lines(value)} onChange={(event) => setValue(parseLines(event.target.value))} placeholder={placeholder} className="mt-1 min-h-20 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" /></label>)}
        </div>
        <div className="mt-3 grid grid-cols-2 gap-3"><label className="text-xs font-medium">Feedback codes (one per line)<textarea value={lines(contract.feedback.codes)} onChange={(event) => update("feedback", { ...contract.feedback, codes: parseLines(event.target.value) })} className="mt-1 min-h-20 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" /></label><label className="text-xs font-medium">Review cycle days<input type="number" min="1" value={contract.evaluation.review_cycle_days ?? ""} onChange={(event) => update("evaluation", { ...contract.evaluation, review_cycle_days: Number(event.target.value) || null })} className="mt-1 w-full rounded-md border border-input bg-background px-3 py-2 text-sm" /></label></div>
        <div className="mt-5 flex justify-end gap-2"><button type="button" onClick={onClose} disabled={saving} className="rounded border border-border px-3 py-2 text-xs">Cancel</button><button type="button" onClick={() => onSave(contract)} disabled={saving} className="rounded bg-primary px-3 py-2 text-xs font-medium text-primary-foreground">{saving ? "Saving…" : "Save local contract"}</button></div>
      </section>
    </div>
  );
}
