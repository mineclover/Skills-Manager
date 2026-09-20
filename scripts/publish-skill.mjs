// Publish the companion `skills-manager-cli` skill to ClawHub, keyed off the
// app version in package.json. Wired into release (npm run publish:skill) and
// into CI on version bumps.
//
// The publish is idempotent by design: we query the slug's latest version on
// ClawHub first and only upload when the local version is strictly newer, so
// re-running on an unchanged version is a no-op (exit 0, nothing sent). This is
// what makes it safe to fire on every push to main.
//
// Env:
//   CLAWHUB_TOKEN   required unless --dry-run — the ClawHub API token
//   SKILL_VERSION   override the version (defaults to package.json version)
//
// Flags:
//   --dry-run   resolve + gate + list files, but never upload (no token needed)
//   --force     publish even if the gate says the version already exists
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { collectSkillFiles, MAX_PUBLISH_TOTAL_BYTES } from "./lib/skill-files.mjs";
import { compareSemver, fetchLatestVersion, parseSemver, publishSkill } from "./lib/clawhub.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const SKILL = {
  dir: join(root, "skills", "skills-manager-cli"),
  slug: "skills-manager-cli",
  displayName: "Skills Manager CLI",
  ownerHandle: "jiweiyeah",
  categories: ["development", "productivity"],
  topics: ["cli", "skills", "skm", "symlinks"],
};

function fail(message) {
  console.error(`[publish:skill] ${message}`);
  process.exit(1);
}

function readAppVersion() {
  if (process.env.SKILL_VERSION?.trim()) {
    return process.env.SKILL_VERSION.trim();
  }
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  return pkg.version;
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const dryRun = args.has("--dry-run");
  const force = args.has("--force");

  const version = readAppVersion();
  if (!parseSemver(version)) {
    fail(`版本号 "${version}" 不是合法的 semver，无法发布`);
  }

  // Fail fast on file-collection problems (missing SKILL.md, oversize) before
  // touching the network — same failures the GUI would surface pre-upload.
  const files = collectSkillFiles(SKILL.dir);
  const totalBytes = files.reduce((sum, file) => sum + file.size, 0);
  console.log(`[publish:skill] ${SKILL.slug} v${version} — ${files.length} 个文件, ${(totalBytes / 1024).toFixed(1)} KB`);
  for (const file of files) {
    console.log(`  · ${file.relPath} (${file.size} B)`);
  }
  if (totalBytes > MAX_PUBLISH_TOTAL_BYTES) {
    fail("技能总大小超过 50MB 上限");
  }

  const lookup = await fetchLatestVersion(SKILL.slug, SKILL.ownerHandle);
  if (lookup.state === "failed") {
    // A failed lookup is unsafe to publish over: we cannot tell "new skill" from
    // "network blip", and guessing wrong either duplicates or clobbers. Bail
    // unless the operator forces it.
    if (!force) {
      fail("无法查询 ClawHub 上的最新版本（网络或服务异常）。加 --force 可跳过该检查强制发布。");
    }
    console.warn("[publish:skill] 版本查询失败，--force 已指定，继续发布");
  } else if (lookup.state === "found") {
    const remote = parseSemver(lookup.version);
    const local = parseSemver(version);
    console.log(`[publish:skill] ClawHub 现有最新版本: v${lookup.version}`);
    if (!force && remote && local && compareSemver(local, remote) <= 0) {
      console.log(`[publish:skill] 本地版本 v${version} 未超过远端 v${lookup.version}，跳过发布（幂等）。`);
      return;
    }
  } else {
    console.log("[publish:skill] ClawHub 上尚未发布过该技能，将首次发布。");
  }

  if (dryRun) {
    console.log("[publish:skill] --dry-run：已通过全部校验，未实际上传。");
    return;
  }

  const token = process.env.CLAWHUB_TOKEN?.trim();
  if (!token) {
    fail("缺少 CLAWHUB_TOKEN 环境变量，无法发布。");
  }

  const changelog = `Skills Manager v${version} 同步发布 skm 配套技能。`;
  let result;
  try {
    result = await publishSkill({
      skillDir: SKILL.dir,
      token,
      slug: SKILL.slug,
      displayName: SKILL.displayName,
      version,
      changelog,
      ownerHandle: SKILL.ownerHandle,
      categories: SKILL.categories,
      topics: SKILL.topics,
    });
  } catch (error) {
    // ClawHub's /versions endpoint omits versions still pending their security
    // scan, so the gate above can miss a version that does exist. The server
    // catches it; treat that as the no-op it is rather than failing the build.
    if (error?.versionAlreadyExists) {
      console.log(`[publish:skill] ${error.message}`);
      console.log("[publish:skill] 该版本已在 ClawHub 上，跳过发布（幂等）。");
      return;
    }
    throw error;
  }

  const status = result.publicationStatus ? ` (${result.publicationStatus})` : "";
  console.log(`[publish:skill] 发布成功 v${result.version}${status}`);
  if (result.externalUrl) {
    console.log(`[publish:skill] ${result.externalUrl}`);
  }
}

main().catch((error) => fail(error?.message ?? String(error)));
