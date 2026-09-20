// Collect the files of a skill directory for a ClawHub publish, mirroring the
// rules the Rust publisher enforces (crates/core/src/services/publish.rs) so the
// CLI/CI path and the GUI path upload the exact same set:
//   - skip any path with a dot-prefixed segment (.git, .env, .DS_Store, ...)
//   - skip node_modules / .clawhub / __pycache__ / .git directories outright
//   - require a top-level SKILL.md
//   - enforce the 10MB per-file and 50MB total ceilings
import { readdirSync, statSync } from "node:fs";
import { extname, join, relative, sep } from "node:path";

/** ClawHub's per-file size ceiling (server MAX_PUBLISH_FILE_BYTES). */
export const MAX_PUBLISH_FILE_BYTES = 10 * 1024 * 1024;
/** ClawHub's per-publish total size ceiling (server MAX_PUBLISH_TOTAL_BYTES). */
export const MAX_PUBLISH_TOTAL_BYTES = 50 * 1024 * 1024;

/** Directory names that are always excluded, matching the Rust ignore set. */
const EXCLUDED_DIRS = new Set([".git", "node_modules", ".clawhub", "__pycache__"]);

/** True when any `/`-separated segment of a POSIX rel path starts with a dot. */
export function hasDotSegment(relPath) {
  return relPath.split("/").some((segment) => segment.startsWith("."));
}

/** True for directory names ClawHub's CLI never uploads. */
export function isExcludedDir(name) {
  return EXCLUDED_DIRS.has(name);
}

/** Best-effort MIME guess from the extension, aligned with the Rust table. */
export function guessContentType(relPath) {
  const ext = extname(relPath).slice(1).toLowerCase();
  const table = {
    md: "text/markdown",
    markdown: "text/markdown",
    txt: "text/plain",
    json: "application/json",
    yaml: "application/yaml",
    yml: "application/yaml",
    toml: "application/toml",
    js: "text/javascript",
    mjs: "text/javascript",
    cjs: "text/javascript",
    ts: "text/typescript",
    py: "text/x-python",
    sh: "application/x-sh",
    bash: "application/x-sh",
    html: "text/html",
    htm: "text/html",
    css: "text/css",
    csv: "text/csv",
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    svg: "image/svg+xml",
    webp: "image/webp",
    pdf: "application/pdf",
    zip: "application/zip",
  };
  return table[ext] ?? "application/octet-stream";
}

function toPosix(relPath) {
  return relPath.split(sep).join("/");
}

function walk(root, current, out) {
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const abs = join(current, entry.name);

    if (entry.isDirectory()) {
      if (isExcludedDir(entry.name) || entry.name.startsWith(".")) {
        continue;
      }
      walk(root, abs, out);
      continue;
    }

    if (!entry.isFile()) {
      continue;
    }

    const relPath = toPosix(relative(root, abs));
    if (hasDotSegment(relPath)) {
      continue;
    }

    const size = statSync(abs).size;
    if (size > MAX_PUBLISH_FILE_BYTES) {
      throw new Error(`文件 "${relPath}" 超过 10MB 单文件上限`);
    }

    out.push({ relPath, absPath: abs, size, contentType: guessContentType(relPath) });
  }
}

/**
 * Collect every publishable file under `root`, sorted by rel path for a stable
 * upload order. Validates the SKILL.md requirement and the total-size ceiling.
 */
export function collectSkillFiles(root) {
  let rootStat;
  try {
    rootStat = statSync(root);
  } catch {
    rootStat = null;
  }
  if (!rootStat || !rootStat.isDirectory()) {
    throw new Error(`技能目录不存在: ${root}`);
  }

  const files = [];
  walk(root, root, files);

  if (files.length === 0) {
    throw new Error("技能目录为空，没有可发布的文件");
  }

  const hasSkillMd = files.some((file) => file.relPath.toLowerCase() === "skill.md");
  if (!hasSkillMd) {
    throw new Error("技能根目录缺少 SKILL.md，ClawHub 要求必须包含该文件");
  }

  const total = files.reduce((sum, file) => sum + file.size, 0);
  if (total > MAX_PUBLISH_TOTAL_BYTES) {
    throw new Error("技能总大小超过 50MB 上限");
  }

  return [...files].sort((a, b) => (a.relPath < b.relPath ? -1 : a.relPath > b.relPath ? 1 : 0));
}
