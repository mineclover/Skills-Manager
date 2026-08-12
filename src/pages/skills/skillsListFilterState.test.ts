import { test } from "node:test";
import assert from "node:assert/strict";

import {
  EMPTY_SKILLS_LIST_FILTER_STATE,
  loadSkillsListFilterState,
  saveSkillsListFilterState,
} from "./skillsListFilterState.ts";

const STORAGE_KEY = "skills-manager:skills-list-filter-state:v1";

function createStorageMock() {
  const storage = new Map<string, string>();
  return {
    storage,
    getItem(key: string) {
      return storage.get(key) ?? null;
    },
    setItem(key: string, value: string) {
      storage.set(key, value);
    },
    removeItem(key: string) {
      storage.delete(key);
    },
  };
}

test("loadSkillsListFilterState restores the saved filters", () => {
  const storageMock = createStorageMock();

  saveSkillsListFilterState(
    {
      selectedTags: ["writing", "review"],
      untaggedOnly: false,
      riskOnly: true,
      favoritesOnly: true,
      scopeFilter: "project",
    },
    storageMock,
  );

  assert.deepEqual(loadSkillsListFilterState(storageMock), {
    selectedTags: ["writing", "review"],
    untaggedOnly: false,
    riskOnly: true,
    favoritesOnly: true,
    scopeFilter: "project",
  });
});

test("loadSkillsListFilterState returns the same value on repeated reads", () => {
  const storageMock = createStorageMock();

  saveSkillsListFilterState(
    { ...EMPTY_SKILLS_LIST_FILTER_STATE, favoritesOnly: true },
    storageMock,
  );

  assert.equal(loadSkillsListFilterState(storageMock).favoritesOnly, true);
  assert.equal(loadSkillsListFilterState(storageMock).favoritesOnly, true);
});

test("saveSkillsListFilterState clears storage when no filter is active", () => {
  const storageMock = createStorageMock();

  saveSkillsListFilterState(
    { ...EMPTY_SKILLS_LIST_FILTER_STATE, untaggedOnly: true },
    storageMock,
  );
  assert.equal(storageMock.storage.has(STORAGE_KEY), true);

  saveSkillsListFilterState(EMPTY_SKILLS_LIST_FILTER_STATE, storageMock);
  assert.equal(storageMock.storage.has(STORAGE_KEY), false);
  assert.deepEqual(loadSkillsListFilterState(storageMock), EMPTY_SKILLS_LIST_FILTER_STATE);
});

test("loadSkillsListFilterState falls back to defaults on malformed values", () => {
  const storageMock = createStorageMock();

  storageMock.setItem(STORAGE_KEY, "not-json");
  assert.deepEqual(loadSkillsListFilterState(storageMock), EMPTY_SKILLS_LIST_FILTER_STATE);

  storageMock.setItem(
    STORAGE_KEY,
    JSON.stringify({ selectedTags: [1, "ok", null], scopeFilter: "nope", riskOnly: "yes" }),
  );
  assert.deepEqual(loadSkillsListFilterState(storageMock), {
    selectedTags: ["ok"],
    untaggedOnly: false,
    riskOnly: false,
    favoritesOnly: false,
    scopeFilter: "all",
  });
});
