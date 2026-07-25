export type SkillsHeaderActionId =
  | "batch-manage"
  | "batch-configure"
  | "project-bindings"
  | "create-skill"
  | "scan-import"
  | "import-skills"
  | "export-skills";

export interface SkillsHeaderActionLayout {
  primaryActionIds: SkillsHeaderActionId[];
  moreActionIds: SkillsHeaderActionId[];
  secondaryActionIds: SkillsHeaderActionId[];
}

export function buildSkillsHeaderActionLayout(
  isBatchManageMode: boolean,
): SkillsHeaderActionLayout {
  if (isBatchManageMode) {
    return {
      primaryActionIds: ["batch-manage", "batch-configure", "export-skills"],
      moreActionIds: [],
      secondaryActionIds: [],
    };
  }
  return {
    primaryActionIds: [],
    moreActionIds: [
      "batch-manage",
      "project-bindings",
      "scan-import",
      "import-skills",
      "export-skills",
    ],
    secondaryActionIds: ["create-skill"],
  };
}
