import type { Skill, SkillMetadata, SkillMetadataMap } from "@/types";
import {
  getSkillMetadataKey,
  migrateSkillMetadataEntryToInstanceId,
} from "./skillTags.ts";

export const SKILL_NOTE_MAX_LENGTH = 1000;

export function normalizeSkillNote(note: string | null | undefined): string {
  return (note ?? "").trim();
}

function isEmptyMetadata(metadata: SkillMetadata): boolean {
  return metadata.tags.length === 0
    && !normalizeSkillNote(metadata.note)
    && metadata.favorited_at == null
    && metadata.publish == null;
}

export function getSkillNoteForSkill(
  skill: Pick<Skill, "id" | "scope" | "instance_id">,
  skillMetadata?: SkillMetadataMap,
): string {
  const instanceNote = normalizeSkillNote(skillMetadata?.[getSkillMetadataKey(skill)]?.note);
  if (instanceNote || skill.scope !== "global") {
    return instanceNote;
  }

  return normalizeSkillNote(skillMetadata?.[skill.id]?.note);
}

export function updateSkillNoteForSkill(
  skill: Pick<Skill, "id" | "scope" | "instance_id">,
  nextNote: string,
  skillMetadata?: SkillMetadataMap,
): SkillMetadataMap {
  const migratedMetadata = migrateSkillMetadataEntryToInstanceId(skill, skillMetadata);
  const metadataKey = getSkillMetadataKey(skill);
  const nextMetadata = { ...migratedMetadata };
  const existing = nextMetadata[metadataKey] ?? { tags: [] };
  const note = normalizeSkillNote(nextNote);
  const { note: _previousNote, ...metadataWithoutNote } = existing;
  const updated: SkillMetadata = note
    ? { ...metadataWithoutNote, note }
    : metadataWithoutNote;

  if (isEmptyMetadata(updated)) {
    delete nextMetadata[metadataKey];
  } else {
    nextMetadata[metadataKey] = updated;
  }

  return nextMetadata;
}
