// Bump the Homebrew cask in jiweiyeah/homebrew-tap to a published release.
//
// Runs from CI on `release: published` (see .github/workflows/update-cask.yml)
// against a checkout of the tap, and works locally too:
//
//   node scripts/update-cask.mjs --cask ../homebrew-tap/Casks/skills-manager.rb
//
// Checksums come from the Releases API `digest` field, so a bump costs two
// small JSON requests instead of downloading ~24MB of DMGs. Older releases
// predate that field, so we fall back to hashing the asset ourselves.
//
// Idempotent: if the cask already carries this version and both checksums
// match, nothing is written and the exit code is still 0 — safe to re-run.
//
// Env:
//   GITHUB_TOKEN   optional — raises the API rate limit; required for private repos
//
// Flags:
//   --cask <path>     path to the cask file (required)
//   --version <ver>   release version without the leading "v" (default: package.json)
//   --repo <o/n>      source repository (default: jiweiyeah/Skills-Manager)
//   --dry-run         print the rewritten cask instead of writing it
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { ARCH_SLUGS, dmgAssetName, readCaskVersion, updateCaskSource } from "./lib/cask.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_REPO = "jiweiyeah/Skills-Manager";

function fail(message) {
  console.error(`[update:cask] ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const flags = { dryRun: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--dry-run") {
      flags.dryRun = true;
    } else if (arg === "--cask" || arg === "--version" || arg === "--repo") {
      const value = argv[i + 1];
      if (!value || value.startsWith("--")) {
        fail(`${arg} 缺少参数值`);
      }
      flags[arg.slice(2)] = value;
      i += 1;
    } else {
      fail(`未知参数: ${arg}`);
    }
  }
  return flags;
}

function githubHeaders() {
  const headers = {
    "User-Agent": "skills-manager-cask-bump",
    Accept: "application/vnd.github+json",
  };
  const token = process.env.GITHUB_TOKEN?.trim();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  return headers;
}

async function fetchRelease(repo, version) {
  const url = `https://api.github.com/repos/${repo}/releases/tags/v${version}`;
  const response = await fetch(url, { headers: githubHeaders() });
  if (response.status === 404) {
    throw new Error(`仓库 ${repo} 上找不到 tag v${version} 的 release（还没发布，或仍是 draft？）`);
  }
  if (!response.ok) {
    throw new Error(`查询 release 失败: HTTP ${response.status} ${response.statusText}`);
  }
  return response.json();
}

async function hashAsset(asset) {
  console.log(`[update:cask] ${asset.name} 无 digest 字段，回退为下载后计算 sha256`);
  const response = await fetch(asset.browser_download_url, { headers: githubHeaders() });
  if (!response.ok) {
    throw new Error(`下载 ${asset.name} 失败: HTTP ${response.status}`);
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  return createHash("sha256").update(buffer).digest("hex");
}

async function resolveChecksums(release, version) {
  const entries = await Promise.all(
    Object.entries(ARCH_SLUGS).map(async ([key, slug]) => {
      const name = dmgAssetName(version, slug);
      const asset = release.assets?.find((candidate) => candidate.name === name);
      if (!asset) {
        const available = (release.assets ?? []).map((a) => a.name).join(", ") || "<无>";
        throw new Error(`release v${version} 里找不到资产 ${name}；现有资产: ${available}`);
      }
      const digest = asset.digest?.startsWith("sha256:") ? asset.digest.slice(7) : null;
      return [key, digest ?? (await hashAsset(asset))];
    }),
  );
  return Object.fromEntries(entries);
}

async function main() {
  const flags = parseArgs(process.argv.slice(2));
  if (!flags.cask) {
    fail("缺少 --cask <path>，不知道要改写哪个 cask 文件");
  }

  const repo = flags.repo ?? DEFAULT_REPO;
  const version =
    flags.version?.replace(/^v/, "") ??
    JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;

  let source;
  try {
    source = readFileSync(flags.cask, "utf8");
  } catch (error) {
    fail(`读取 cask 文件失败 (${flags.cask}): ${error.message}`);
  }

  const release = await fetchRelease(repo, version);
  const sha256 = await resolveChecksums(release, version);

  const updated = updateCaskSource(source, { version, sha256 });
  if (updated === source) {
    console.log(`[update:cask] cask 已是 v${version} 且校验和一致，无需改动（幂等）。`);
    return;
  }

  console.log(`[update:cask] ${readCaskVersion(source) ?? "<未知>"} -> ${version}`);
  console.log(`[update:cask]   arm   ${sha256.arm}`);
  console.log(`[update:cask]   intel ${sha256.intel}`);

  if (flags.dryRun) {
    console.log("[update:cask] --dry-run：未写入文件。改写后的内容如下：\n");
    console.log(updated);
    return;
  }

  writeFileSync(flags.cask, updated);
  console.log(`[update:cask] 已写入 ${flags.cask}`);
}

main().catch((error) => fail(error?.message ?? String(error)));
