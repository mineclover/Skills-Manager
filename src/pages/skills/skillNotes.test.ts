import { test } from "node:test";
import assert from "node:assert/strict";

import type { Skill } from "../../types/index.ts";
import {
  getSkillNoteForSkill,
  normalizeSkillNote,
  updateSkillNoteForSkill,
} from "./skillNotes.ts";

const skill: Skill = {
  id: "react-playground",
  instance_id: "global:react-playground",
  scope: "global",
  project_id: null,
  project_name: null,
  name: "React Playground",
  description: null,
  version: "1.0.0",
  source: "local",
  enabled: {},
  path: "/tmp/react-playground",
};

test("normalizeSkillNote trims surrounding whitespace and preserves line breaks", () => {
  assert.equal(normalizeSkillNote("  第一行\n第二行  "), "第一行\n第二行");
});

test("getSkillNoteForSkill reads instance metadata and supports legacy global keys", () => {
  assert.equal(getSkillNoteForSkill(skill, {
    "global:react-playground": { tags: [], note: "常用前端检查" },
  }), "常用前端检查");
  assert.equal(getSkillNoteForSkill(skill, {
    "react-playground": { tags: [], note: "旧版备注" },
  }), "旧版备注");
});

test("updateSkillNoteForSkill preserves tags, favorite, and publish metadata", () => {
  const publish = {
    slug: "react-playground",
    version: "1.0.0",
    published_at: 1,
  };
  const result = updateSkillNoteForSkill(skill, "  我自己的说明  ", {
    "global:react-playground": {
      tags: ["react"],
      favorited_at: 123,
      publish,
    },
  });

  assert.deepEqual(result["global:react-playground"], {
    tags: ["react"],
    note: "我自己的说明",
    favorited_at: 123,
    publish,
  });
});

test("clearing a note keeps other metadata and removes a note-only entry", () => {
  const withTag = updateSkillNoteForSkill(skill, "", {
    "global:react-playground": { tags: ["react"], note: "旧备注" },
  });
  assert.deepEqual(withTag["global:react-playground"], { tags: ["react"] });

  const noteOnly = updateSkillNoteForSkill(skill, "   ", {
    "global:react-playground": { tags: [], note: "旧备注" },
  });
  assert.equal(noteOnly["global:react-playground"], undefined);
});
