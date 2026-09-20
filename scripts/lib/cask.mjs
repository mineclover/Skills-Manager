// Pure helpers for rewriting the Homebrew cask that distributes the macOS app
// (see jiweiyeah/homebrew-tap). Kept free of I/O so update-cask.test.mjs can
// exercise the rewrite rules without a network or a checked-out tap.
//
// Every rewrite is verified rather than best-effort: a regex that silently
// fails to match would produce a cask still pointing at the previous version
// while the workflow reports success, which is the worst possible outcome for
// a release pipeline. Callers get a thrown error instead.

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

// GitHub turns the space in "Skills Manager_..." into a dot for release asset
// names. The cask's `arch` stanza maps arm -> aarch64 and intel -> x64.
export const ARCH_SLUGS = { arm: "aarch64", intel: "x64" };

export function dmgAssetName(version, archSlug) {
  return `Skills.Manager_${version}_${archSlug}.dmg`;
}

function replaceOnce(source, pattern, replacement, label) {
  const matches = source.match(new RegExp(pattern.source, `${pattern.flags}g`));
  if (!matches) {
    throw new Error(`cask 文件里找不到 ${label}，无法安全改写（模板是否已变动？）`);
  }
  if (matches.length > 1) {
    throw new Error(`cask 文件里的 ${label} 匹配到 ${matches.length} 处，拒绝改写以免改错`);
  }
  return source.replace(pattern, replacement);
}

export function readCaskVersion(source) {
  return source.match(/^\s*version\s+"([^"]*)"/m)?.[1] ?? null;
}

/**
 * Returns the cask source with version and both architecture checksums swapped.
 * Throws if any anchor is missing/ambiguous or a checksum is malformed.
 */
export function updateCaskSource(source, { version, sha256 }) {
  if (!version?.trim()) {
    throw new Error("缺少 version");
  }
  for (const key of ["arm", "intel"]) {
    if (!SHA256_PATTERN.test(sha256?.[key] ?? "")) {
      throw new Error(`${key} 的 sha256 不是合法的 64 位十六进制值: ${sha256?.[key]}`);
    }
  }

  const withVersion = replaceOnce(
    source,
    /^(\s*version\s+)"[^"]*"/m,
    `$1"${version}"`,
    "version 行",
  );
  const withArm = replaceOnce(
    withVersion,
    /^(\s*sha256\s+arm:\s+)"[^"]*"/m,
    `$1"${sha256.arm}"`,
    "sha256 arm 行",
  );
  return replaceOnce(
    withArm,
    /^(\s*intel:\s+)"[^"]*"/m,
    `$1"${sha256.intel}"`,
    "sha256 intel 行",
  );
}
