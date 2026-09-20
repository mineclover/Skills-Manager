import type { ProjectBinding } from "@/types";

function normalizePathSeparators(path: string): string {
  return path.replace(/\\+/g, "/");
}

function normalizeProjectRootPath(directoryPath: string): string {
  const trimmed = normalizePathSeparators(directoryPath.trim());
  if (!trimmed) {
    throw new Error("Project root path is required");
  }

  if (trimmed === "/" || /^[A-Za-z]:\/$/.test(trimmed)) {
    throw new Error("Select a project directory instead of the filesystem root");
  }

  return trimmed.replace(/\/+$/, "") || "/";
}

function getPathSegments(directoryPath: string): string[] {
  return directoryPath.split("/").filter(Boolean);
}

export function buildDefaultProjectNameFromRootPath(rootPath: string): string {
  const normalizedRootPath = normalizeProjectRootPath(rootPath);
  const segments = getPathSegments(normalizedRootPath);
  return segments[segments.length - 1] ?? normalizedRootPath;
}

function slugifyProjectName(projectName: string): string {
  return projectName
    .trim()
    .toLowerCase()
    .replace(/\s+/g, "-")
    .replace(/[^a-z0-9._-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

function buildPathHash(skillsDir: string): string {
  let hash = 2166136261;
  for (let index = 0; index < skillsDir.length; index += 1) {
    hash ^= skillsDir.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

function buildProjectId(rootPath: string, projectName: string): string {
  const slug = slugifyProjectName(projectName);
  const suffix = buildPathHash(rootPath);
  return slug ? `${slug}-${suffix}` : `project-${suffix}`;
}

function normalizeProjectName(projectName: string): string {
  const normalizedName = projectName.trim();
  if (!normalizedName) {
    throw new Error("Project name is required");
  }
  return normalizedName;
}

export function buildProjectBindingFromRootPath(
  rootPath: string,
  projectName?: string,
): ProjectBinding {
  const normalizedRootPath = normalizeProjectRootPath(rootPath);
  const resolvedProjectName = projectName
    ? normalizeProjectName(projectName)
    : buildDefaultProjectNameFromRootPath(normalizedRootPath);

  return {
    id: buildProjectId(normalizedRootPath, buildDefaultProjectNameFromRootPath(normalizedRootPath)),
    name: resolvedProjectName,
    root_path: normalizedRootPath,
    skills_dir: `${normalizedRootPath}/.skills-manager/skills`,
  };
}

export function hasProjectRootConflict(
  projects: ProjectBinding[] | null | undefined,
  nextProject: ProjectBinding,
): boolean {
  return (projects ?? []).some((project) => project.root_path === nextProject.root_path);
}

export function resolveActiveProjectId(
  activeProjectId: string | null | undefined,
  projects: ProjectBinding[] | null | undefined,
): string | null {
  if (!activeProjectId) {
    return null;
  }

  return (projects ?? []).some((project) => project.id === activeProjectId)
    ? activeProjectId
    : null;
}

export function resolveNextActiveProjectIdAfterAddition(
  activeProjectId: string | null | undefined,
  projects: ProjectBinding[] | null | undefined,
  nextProject: ProjectBinding,
): string {
  const nextProjects = [...(projects ?? []), nextProject];
  return resolveActiveProjectId(activeProjectId, nextProjects) ?? nextProject.id;
}

export function resolveNextProjectBindingsAfterRemoval(
  projects: ProjectBinding[] | null | undefined,
  projectIdToRemove: string,
  activeProjectId: string | null | undefined,
): { projects: ProjectBinding[]; activeProjectId: string | null } {
  const currentProjects = projects ?? [];
  const nextProjects = currentProjects.filter((project) => project.id !== projectIdToRemove);

  if (nextProjects.length === currentProjects.length) {
    return {
      projects: currentProjects,
      activeProjectId: resolveActiveProjectId(activeProjectId, currentProjects),
    };
  }

  return {
    projects: nextProjects,
    activeProjectId: activeProjectId === projectIdToRemove
      ? null
      : resolveActiveProjectId(activeProjectId, nextProjects),
  };
}
