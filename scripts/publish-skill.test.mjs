import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { collectSkillFiles, guessContentType, hasDotSegment, isExcludedDir } from "../scripts/lib/skill-files.mjs";
import {
  buildPublishPayload,
  compareSemver,
  fetchLatestVersion,
  isVersionAlreadyExists,
  parseSemver,
  publishSkill,
  sanitizeSlug,
  verifyToken,
} from "../scripts/lib/clawhub.mjs";

function withTempSkill(build) {
  const dir = mkdtempSync(join(tmpdir(), "skill-publish-"));
  try {
    build(dir);
    return collectSkillFiles(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("hasDotSegment flags hidden paths only", () => {
  assert.equal(hasDotSegment(".env"), true);
  assert.equal(hasDotSegment("scripts/.secret/key.txt"), true);
  assert.equal(hasDotSegment("scripts/run.sh"), false);
  assert.equal(hasDotSegment("SKILL.md"), false);
});

test("isExcludedDir matches the Rust ignore set", () => {
  for (const name of [".git", "node_modules", ".clawhub", "__pycache__"]) {
    assert.equal(isExcludedDir(name), true);
  }
  assert.equal(isExcludedDir("references"), false);
});

test("guessContentType covers common skill files", () => {
  assert.equal(guessContentType("SKILL.md"), "text/markdown");
  assert.equal(guessContentType("scripts/run.sh"), "application/x-sh");
  assert.equal(guessContentType("data/x.bin"), "application/octet-stream");
});

test("collectSkillFiles skips hidden and excluded paths, sorted", () => {
  const files = withTempSkill((dir) => {
    writeFileSync(join(dir, "SKILL.md"), "---\nname: demo\n---\n");
    mkdirSync(join(dir, "references"));
    writeFileSync(join(dir, "references", "tools.md"), "tools");
    mkdirSync(join(dir, "node_modules", "pkg"), { recursive: true });
    writeFileSync(join(dir, "node_modules", "pkg", "index.js"), "x");
    mkdirSync(join(dir, ".git"));
    writeFileSync(join(dir, ".git", "config"), "x");
    writeFileSync(join(dir, ".env"), "SECRET=1");
  });
  assert.deepEqual(files.map((file) => file.relPath), ["SKILL.md", "references/tools.md"]);
});

test("collectSkillFiles requires SKILL.md", () => {
  assert.throws(
    () =>
      withTempSkill((dir) => {
        writeFileSync(join(dir, "README.md"), "nope");
      }),
    /SKILL\.md/,
  );
});

test("sanitizeSlug normalizes arbitrary names", () => {
  assert.equal(sanitizeSlug("My Skill"), "my-skill");
  assert.equal(sanitizeSlug("  Git Worktree Cleanup "), "git-worktree-cleanup");
  assert.equal(sanitizeSlug("a__b--c"), "a-b-c");
  assert.equal(sanitizeSlug("---"), "");
  assert.equal(sanitizeSlug("PDF2Text"), "pdf2text");
});

test("parseSemver / compareSemver order releases", () => {
  assert.equal(parseSemver("1.2.3").raw, "1.2.3");
  assert.equal(parseSemver("not-semver"), null);
  assert.equal(compareSemver(parseSemver("2.1.11"), parseSemver("2.1.9")) > 0, true);
  assert.equal(compareSemver(parseSemver("2.1.10"), parseSemver("2.1.10")), 0);
});

test("buildPublishPayload always carries a tags array and trims fields", () => {
  const payload = buildPublishPayload({
    slug: "demo",
    displayName: "  Demo Skill  ",
    version: " 1.2.3 ",
    changelog: " first ",
  });
  assert.deepEqual(payload.tags, ["latest"]);
  assert.equal(payload.displayName, "Demo Skill");
  assert.equal(payload.version, "1.2.3");
  assert.equal(payload.changelog, "first");
  assert.equal(payload.acceptLicenseTerms, true);
  assert.equal("ownerHandle" in payload, false);
  assert.equal("categories" in payload, false);
  assert.equal("topics" in payload, false);
});

test("buildPublishPayload strips a leading @ and includes taxonomy", () => {
  const payload = buildPublishPayload({
    slug: "demo",
    displayName: "Demo",
    version: "1.0.0",
    changelog: "",
    ownerHandle: "@my-org",
    categories: ["development"],
    topics: ["git", "worktree"],
  });
  assert.equal(payload.ownerHandle, "my-org");
  assert.deepEqual(payload.categories, ["development"]);
  assert.deepEqual(payload.topics, ["git", "worktree"]);
});

test("fetchLatestVersion returns the max published semver", async () => {
  const fetchImpl = async () => ({
    status: 200,
    ok: true,
    json: async () => ({ versions: [{ version: "1.0.0" }, { version: "1.2.0" }, { version: "1.1.5" }] }),
  });
  assert.deepEqual(await fetchLatestVersion("demo", "owner", { fetchImpl }), { state: "found", version: "1.2.0" });
});

// The live API answers with `items`, not `versions`. Reading the wrong key made
// an existing skill look unpublished, which then tried to re-publish a version
// the server already had.
test("fetchLatestVersion reads the live `items` shape", async () => {
  const fetchImpl = async () => ({
    status: 200,
    ok: true,
    json: async () => ({
      items: [{ version: "2.1.10", createdAt: 1787589291509, changelog: "x" }],
      nextCursor: null,
    }),
  });
  assert.deepEqual(await fetchLatestVersion("skills-manager-cli", "jiweiyeah", { fetchImpl }), {
    state: "found",
    version: "2.1.10",
  });
});

test("fetchLatestVersion follows nextCursor pagination", async () => {
  const pages = [
    { items: [{ version: "1.0.0" }], nextCursor: "c1" },
    { items: [{ version: "3.2.1" }], nextCursor: null },
  ];
  let call = 0;
  const seen = [];
  const fetchImpl = async (url) => {
    seen.push(url);
    const body = pages[call];
    call += 1;
    return { status: 200, ok: true, json: async () => body };
  };
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl }), { state: "found", version: "3.2.1" });
  assert.equal(call, 2);
  assert.equal(seen[1].includes("cursor=c1"), true);
});

// An unrecognized body must not read as "never published" — that is the failure
// mode that causes a duplicate-version publish.
test("fetchLatestVersion treats an unknown body shape as failure", async () => {
  const fetchImpl = async () => ({ status: 200, ok: true, json: async () => ({ data: "unexpected" }) });
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl }), { state: "failed" });
});

test("fetchLatestVersion distinguishes not-published (404) from failure", async () => {
  const notFound = async () => ({ status: 404, ok: false, json: async () => ({}) });
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl: notFound }), { state: "not_published" });

  const boom = async () => {
    throw new Error("network down");
  };
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl: boom }), { state: "failed" });
});

test("fetchLatestVersion treats an empty version list as not-published", async () => {
  const fetchImpl = async () => ({ status: 200, ok: true, json: async () => ({ versions: [] }) });
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl }), { state: "not_published" });

  const emptyItems = async () => ({ status: 200, ok: true, json: async () => ({ items: [], nextCursor: null }) });
  assert.deepEqual(await fetchLatestVersion("demo", null, { fetchImpl: emptyItems }), { state: "not_published" });
});

test("verifyToken rejects an empty token without a request", async () => {
  await assert.rejects(() => verifyToken("   "), /尚未配置/);
});

test("verifyToken maps 401 to an invalid-token error", async () => {
  const fetchImpl = async () => ({ status: 401, ok: false });
  await assert.rejects(() => verifyToken("clh_x", { fetchImpl }), /无效或已过期/);
});

test("isVersionAlreadyExists recognizes ClawHub's duplicate-version reply", () => {
  assert.equal(
    isVersionAlreadyExists("Version 2.1.10 already exists. Increment the version number and try again."),
    true,
  );
  assert.equal(isVersionAlreadyExists("Publish payload: tags: an array"), false);
  assert.equal(isVersionAlreadyExists(""), false);
  assert.equal(isVersionAlreadyExists(undefined), false);
});

// ClawHub's /versions endpoint omits versions still pending their security
// scan, so the version gate can miss a version the server already has. That
// must surface as a flagged no-op, not an opaque failure.
test("publishSkill flags a duplicate version instead of a generic failure", async () => {
  const dir = mkdtempSync(join(tmpdir(), "skill-publish-dup-"));
  try {
    writeFileSync(join(dir, "SKILL.md"), "---\nname: demo\n---\n");
    const fetchImpl = async () => ({
      status: 400,
      ok: false,
      text: async () => "Version 2.1.10 already exists. Increment the version number and try again.",
    });
    await assert.rejects(
      () =>
        publishSkill(
          { skillDir: dir, token: "clh_x", slug: "demo", displayName: "Demo", version: "2.1.10", changelog: "" },
          { fetchImpl },
        ),
      (error) => error.versionAlreadyExists === true,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("publishSkill rejects an invalid semver before touching the network", async () => {
  let called = false;
  const fetchImpl = async () => {
    called = true;
    return { status: 200, ok: true, json: async () => ({}) };
  };
  await assert.rejects(
    () =>
      publishSkill(
        { skillDir: ".", token: "clh_x", slug: "demo", displayName: "Demo", version: "2.1", changelog: "" },
        { fetchImpl },
      ),
    /semver/,
  );
  assert.equal(called, false);
});
