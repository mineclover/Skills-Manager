import assert from "node:assert/strict";
import test from "node:test";
import type { ProjectBinding } from "../types";

import {
  buildProjectBindingFromRootPath,
  hasProjectRootConflict,
  resolveActiveProjectId,
  resolveNextActiveProjectIdAfterAddition,
  resolveNextProjectBindingsAfterRemoval,
} from "./projectBindings.ts";

test("buildProjectBindingFromRootPath derives the managed store from the selected root", () => {
  const binding = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");

  assert.equal(binding.name, "project-alpha");
  assert.equal(binding.root_path, "/Users/yjw/code/project-alpha");
  assert.equal(binding.skills_dir, "/Users/yjw/code/project-alpha/.skills-manager/skills");
  assert.match(binding.id, /^project-alpha-[a-z0-9]+$/);
});

test("buildProjectBindingFromRootPath normalizes trailing slashes without changing its id", () => {
  const plain = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const trailing = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha///");

  assert.deepEqual(trailing, plain);
});

test("buildProjectBindingFromRootPath supports Windows project roots", () => {
  const binding = buildProjectBindingFromRootPath("C:\\Users\\yjw\\code\\project-alpha");

  assert.equal(binding.name, "project-alpha");
  assert.equal(binding.root_path, "C:/Users/yjw/code/project-alpha");
  assert.equal(binding.skills_dir, "C:/Users/yjw/code/project-alpha/.skills-manager/skills");
});

test("buildProjectBindingFromRootPath derives a stable fallback id for non-ascii names", () => {
  const binding = buildProjectBindingFromRootPath("/Users/yjw/code/项目技能管理");

  assert.equal(binding.name, "项目技能管理");
  assert.match(binding.id, /^project-[a-z0-9]+$/);
});

test("buildProjectBindingFromRootPath uses different ids for same-name roots", () => {
  const first = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const second = buildProjectBindingFromRootPath("/Users/archive/project-alpha");

  assert.notEqual(first.id, second.id);
  assert.equal(first.name, second.name);
});

test("buildProjectBindingFromRootPath accepts a custom display name without changing root identity", () => {
  const defaultBinding = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const namedBinding = buildProjectBindingFromRootPath(
    "/Users/yjw/code/project-alpha",
    "Alpha Workspace",
  );

  assert.equal(namedBinding.name, "Alpha Workspace");
  assert.equal(namedBinding.id, defaultBinding.id);
});

test("buildProjectBindingFromRootPath rejects empty and filesystem-root paths", () => {
  assert.throws(
    () => buildProjectBindingFromRootPath("   "),
    /Project root path is required/,
  );
  assert.throws(
    () => buildProjectBindingFromRootPath("/"),
    /Select a project directory instead of the filesystem root/,
  );
  assert.throws(
    () => buildProjectBindingFromRootPath("C:\\"),
    /Select a project directory instead of the filesystem root/,
  );
});

test("hasProjectRootConflict compares normalized project roots", () => {
  const existing = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const sameRoot = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha///");
  const differentRoot = buildProjectBindingFromRootPath("/Users/archive/project-alpha");

  assert.equal(hasProjectRootConflict([existing], sameRoot), true);
  assert.equal(hasProjectRootConflict([existing], differentRoot), false);
});

test("resolveActiveProjectId rejects stale ids and preserves known ids", () => {
  const existing = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");

  assert.equal(resolveActiveProjectId("missing-project", [existing]), null);
  assert.equal(resolveActiveProjectId(existing.id, [existing]), existing.id);
});

test("resolveNextActiveProjectIdAfterAddition selects new project only when current id is stale", () => {
  const existing = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const next = buildProjectBindingFromRootPath("/Users/yjw/code/project-beta");

  assert.equal(resolveNextActiveProjectIdAfterAddition("missing", [], next), next.id);
  assert.equal(
    resolveNextActiveProjectIdAfterAddition(existing.id, [existing], next),
    existing.id,
  );
});

test("resolveNextProjectBindingsAfterRemoval clears the active project when removed", () => {
  const first = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const second = buildProjectBindingFromRootPath("/Users/yjw/code/project-beta");

  assert.deepEqual(
    resolveNextProjectBindingsAfterRemoval([first, second], second.id, second.id),
    { projects: [first], activeProjectId: null },
  );
});

test("resolveNextProjectBindingsAfterRemoval preserves another active project", () => {
  const first = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const second = buildProjectBindingFromRootPath("/Users/yjw/code/project-beta");

  assert.deepEqual(
    resolveNextProjectBindingsAfterRemoval([first, second], second.id, first.id),
    { projects: [first], activeProjectId: first.id },
  );
});

test("resolveNextProjectBindingsAfterRemoval ignores unknown ids", () => {
  const first = buildProjectBindingFromRootPath("/Users/yjw/code/project-alpha");
  const second = buildProjectBindingFromRootPath("/Users/yjw/code/project-beta");
  const projects: ProjectBinding[] = [first, second];

  assert.deepEqual(
    resolveNextProjectBindingsAfterRemoval(projects, "missing-project", first.id),
    { projects, activeProjectId: first.id },
  );
});
