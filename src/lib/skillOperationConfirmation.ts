import type { SkillBindingImpact, SkillOperationPreview } from "@/types";

export function dedupeSkillBindingImpacts(impacts: SkillBindingImpact[]): SkillBindingImpact[] {
  return Array.from(new Map(impacts.map((impact) => [
    JSON.stringify([impact.provider_id, impact.root_path ?? null]),
    impact,
  ])).values());
}

export function formatSkillBindingImpacts(impacts: SkillBindingImpact[], separator = "\n"): string {
  return dedupeSkillBindingImpacts(impacts)
    .map((impact) => impact.root_path
      ? `${impact.display_name} (${impact.root_path})`
      : impact.display_name)
    .join(separator);
}

/** null cancels; false proceeds without shared consent; true records accepted shared consent. */
export async function confirmSharedOperationPreviews(
  previews: SkillOperationPreview[],
  confirmMessage: (message: string) => Promise<boolean>,
  fallbackWarning: string,
): Promise<boolean | null> {
  const sharedPreviews = previews.filter((preview) => preview.requires_confirmation);
  if (sharedPreviews.length === 0) {
    return false;
  }

  const warnings = Array.from(new Set(sharedPreviews.map((preview) => preview.warning || fallbackWarning)));
  const impacts = formatSkillBindingImpacts(sharedPreviews.flatMap((preview) => preview.impacts));
  const message = `${warnings.join("\n")}${impacts ? `\n\n${impacts}` : ""}`;
  return await confirmMessage(message) ? true : null;
}
