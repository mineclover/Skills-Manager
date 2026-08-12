const SKILLS_LIST_FILTER_STATE_KEY = "skills-manager:skills-list-filter-state:v1";

type FilterStateStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export type SkillsScopeFilter = "all" | "global" | "project" | "tool";

export interface SkillsListFilterState {
  selectedTags: string[];
  untaggedOnly: boolean;
  riskOnly: boolean;
  favoritesOnly: boolean;
  scopeFilter: SkillsScopeFilter;
}

export const EMPTY_SKILLS_LIST_FILTER_STATE: SkillsListFilterState = {
  selectedTags: [],
  untaggedOnly: false,
  riskOnly: false,
  favoritesOnly: false,
  scopeFilter: "all",
};

const SCOPE_FILTERS: readonly SkillsScopeFilter[] = ["all", "global", "project", "tool"];

function getFilterStateStorage(storage?: FilterStateStorage): FilterStateStorage | null {
  if (storage) {
    return storage;
  }

  if (typeof sessionStorage === "undefined") {
    return null;
  }

  return sessionStorage;
}

function isFilterStateEmpty(state: SkillsListFilterState): boolean {
  return (
    state.selectedTags.length === 0 &&
    !state.untaggedOnly &&
    !state.riskOnly &&
    !state.favoritesOnly &&
    state.scopeFilter === "all"
  );
}

function parseScopeFilter(value: unknown): SkillsScopeFilter {
  return SCOPE_FILTERS.includes(value as SkillsScopeFilter)
    ? (value as SkillsScopeFilter)
    : "all";
}

function parseTags(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((tag): tag is string => typeof tag === "string") : [];
}

/**
 * Persists the Skills list filters for the current session so navigating to the
 * editor (or any other page) and coming back keeps the user's filtered view.
 * Session-scoped on purpose: a fresh app launch always starts unfiltered.
 */
export function saveSkillsListFilterState(
  state: SkillsListFilterState,
  storage?: FilterStateStorage,
) {
  const targetStorage = getFilterStateStorage(storage);
  if (!targetStorage) {
    return;
  }

  try {
    if (isFilterStateEmpty(state)) {
      targetStorage.removeItem(SKILLS_LIST_FILTER_STATE_KEY);
      return;
    }

    targetStorage.setItem(SKILLS_LIST_FILTER_STATE_KEY, JSON.stringify(state));
  } catch {
    // Session storage is optional; the in-memory filters still work.
  }
}

export function loadSkillsListFilterState(
  storage?: FilterStateStorage,
): SkillsListFilterState {
  const targetStorage = getFilterStateStorage(storage);
  if (!targetStorage) {
    return EMPTY_SKILLS_LIST_FILTER_STATE;
  }

  try {
    const rawValue = targetStorage.getItem(SKILLS_LIST_FILTER_STATE_KEY);
    if (!rawValue) {
      return EMPTY_SKILLS_LIST_FILTER_STATE;
    }

    const parsed: unknown = JSON.parse(rawValue);
    if (typeof parsed !== "object" || parsed === null) {
      return EMPTY_SKILLS_LIST_FILTER_STATE;
    }

    const record = parsed as Record<string, unknown>;
    return {
      selectedTags: parseTags(record.selectedTags),
      untaggedOnly: record.untaggedOnly === true,
      riskOnly: record.riskOnly === true,
      favoritesOnly: record.favoritesOnly === true,
      scopeFilter: parseScopeFilter(record.scopeFilter),
    };
  } catch {
    return EMPTY_SKILLS_LIST_FILTER_STATE;
  }
}
