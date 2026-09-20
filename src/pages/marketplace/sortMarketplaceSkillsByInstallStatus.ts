import type { MarketplaceSkill } from "../../types";

export function sortMarketplaceSkillsByInstallStatus(
  skills: MarketplaceSkill[],
): MarketplaceSkill[] {
  return skills
    .map((skill, index) => ({ skill, index }))
    .sort((a, b) => {
      const installRankDiff =
        getMarketplaceInstallStatusRank(a.skill.install_status)
        - getMarketplaceInstallStatusRank(b.skill.install_status);
      if (installRankDiff !== 0) {
        return installRankDiff;
      }

      return a.index - b.index;
    })
    .map(({ skill }) => skill);
}

function getMarketplaceInstallStatusRank(
  status: MarketplaceSkill["install_status"],
): number {
  if (status === "update_available") return 0;
  if (status === "installed") return 1;
  return 2;
}
