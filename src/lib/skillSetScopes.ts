interface WorkScopeContext {
  work_scope?: string;
  work_scope_tags?: string[];
}

function normalizeTags(tags: string[]): string[] {
  return [...new Set(tags.map((tag) => tag.trim()).filter(Boolean))];
}

export function parseWorkScopeTags(input: string): string[] {
  return normalizeTags(input.split(","));
}

export function getWorkScopeTags(context: WorkScopeContext): string[] {
  // An explicit empty array means every scope. Legacy scalar values stay one tag,
  // even when their names contain commas.
  return normalizeTags(context.work_scope_tags ?? [context.work_scope ?? ""]);
}

export function describeWorkScopeTags(context: WorkScopeContext): string {
  return getWorkScopeTags(context).join(" + ")
    || (context.work_scope_tags === undefined ? "Scope not specified" : "All work scopes");
}
