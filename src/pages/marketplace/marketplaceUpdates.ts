import type { MarketplaceInstallation, MarketplaceSkill } from "@/types";

export interface MarketplaceUpdateCandidate {
  skill: MarketplaceSkill;
  installations: MarketplaceInstallation[];
}

export function getMarketplaceUpdateCandidates(
  skills: MarketplaceSkill[],
): MarketplaceUpdateCandidate[] {
  return skills.flatMap((skill) => {
    const installations = (skill.installations ?? []).filter(
      (installation) => installation.install_status === "update_available",
    );
    return installations.length > 0 ? [{ skill, installations }] : [];
  });
}

export function countMarketplaceUpdateInstallations(
  candidates: MarketplaceUpdateCandidate[],
): number {
  return candidates.reduce((count, candidate) => count + candidate.installations.length, 0);
}
