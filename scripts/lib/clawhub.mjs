// A minimal ClawHub API client for publishing a skill from Node (CI or local),
// mirroring crates/core/src/services/publish.rs so the CLI/CI path stays in
// lockstep with the GUI publisher: same endpoints, same multipart payload, same
// version gate. Kept dependency-free — Node 18+ ships fetch/FormData/Blob.
import { readFileSync } from "node:fs";

import { collectSkillFiles } from "./skill-files.mjs";

export const CLAWHUB_API_BASE = "https://clawhub.ai/api/v1";
export const CLAWHUB_SITE_ORIGIN = "https://clawhub.ai";

const QUERY_TIMEOUT_MS = 20_000;
const PUBLISH_TIMEOUT_MS = 120_000;

async function request(url, options, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } catch (error) {
    if (error?.name === "AbortError") {
      throw new Error(`请求 ClawHub 超时: ${url}`);
    }
    throw new Error(`连接 ClawHub 失败: ${error?.message ?? error}`);
  } finally {
    clearTimeout(timer);
  }
}

/** Normalize an arbitrary name into a ClawHub slug (lowercase, [a-z0-9-]). */
export function sanitizeSlug(input) {
  let slug = "";
  let prevDash = false;
  for (const ch of (input ?? "").trim()) {
    if (/[a-z0-9]/i.test(ch)) {
      slug += ch.toLowerCase();
      prevDash = false;
    } else if (!prevDash && slug.length > 0) {
      slug += "-";
      prevDash = true;
    }
  }
  return slug.replace(/^-+|-+$/g, "");
}

/** Verify the token and return { handle, displayName, image } (throws if bad). */
export async function verifyToken(token, { fetchImpl } = {}) {
  const trimmed = (token ?? "").trim();
  if (!trimmed) {
    throw new Error("尚未配置 ClawHub API token");
  }

  const doFetch = fetchImpl ?? ((url, opts) => request(url, opts, QUERY_TIMEOUT_MS));
  const response = await doFetch(`${CLAWHUB_API_BASE}/whoami`, {
    headers: { Authorization: `Bearer ${trimmed}`, "User-Agent": "skills-manager" },
  });

  if (response.status === 401) {
    throw new Error("ClawHub token 无效或已过期");
  }
  if (!response.ok) {
    throw new Error(`ClawHub 返回错误状态: ${response.status}`);
  }

  const body = await response.json();
  const user = body?.user ?? {};
  return { handle: user.handle ?? null, displayName: user.displayName ?? null, image: user.image ?? null };
}

/**
 * Look up the latest published semver for a slug.
 * Returns { state: "found", version } | { state: "not_published" } | { state: "failed" }.
 * The three-way result mirrors the Rust VersionLookup: "not published" and
 * "lookup failed" must stay distinct so a network blip never re-publishes an
 * existing skill as if it were brand new.
 */
export async function fetchLatestVersion(slug, owner, { fetchImpl } = {}) {
  const doFetch = fetchImpl ?? ((url, opts) => request(url, opts, QUERY_TIMEOUT_MS));
  const ownerQuery = owner && owner.trim() ? `owner=${encodeURIComponent(owner.trim())}` : "";
  const parsed = [];
  let cursor = null;
  // The list is paginated; walk it so a skill with many releases still yields
  // the true maximum. Bounded to keep a malformed cursor from looping forever.
  for (let page = 0; page < 20; page += 1) {
    const params = [ownerQuery, cursor ? `cursor=${encodeURIComponent(cursor)}` : ""].filter(Boolean).join("&");
    const url = `${CLAWHUB_API_BASE}/skills/${encodeURIComponent(slug)}/versions${params ? `?${params}` : ""}`;

    let response;
    try {
      response = await doFetch(url, { headers: { "User-Agent": "skills-manager" } });
    } catch {
      return { state: "failed" };
    }

    if (response.status === 404) {
      return { state: "not_published" };
    }
    if (!response.ok) {
      return { state: "failed" };
    }

    let body;
    try {
      body = await response.json();
    } catch {
      return { state: "failed" };
    }

    // ClawHub returns `items`; older/other shapes used `versions`. Accept both
    // rather than silently reading an absent key — treating "published" as
    // "never published" is exactly how a duplicate-version publish happens.
    const list = Array.isArray(body?.items)
      ? body.items
      : Array.isArray(body?.versions)
        ? body.versions
        : null;
    if (list === null) {
      return { state: "failed" };
    }

    for (const entry of list) {
      const semver = parseSemver(entry?.version);
      if (semver) {
        parsed.push(semver);
      }
    }

    cursor = body?.nextCursor ?? null;
    if (!cursor) {
      break;
    }
  }

  if (parsed.length === 0) {
    return { state: "not_published" };
  }
  parsed.sort(compareSemver);
  const max = parsed[parsed.length - 1];
  return { state: "found", version: max.raw };
}

/** Parse a strict `major.minor.patch` semver; returns null on anything else. */
export function parseSemver(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec((value ?? "").trim());
  if (!match) {
    return null;
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    raw: `${Number(match[1])}.${Number(match[2])}.${Number(match[3])}`,
  };
}

/** Order comparator for parsed semvers. */
export function compareSemver(a, b) {
  return a.major - b.major || a.minor - b.minor || a.patch - b.patch;
}

/**
 * True when ClawHub rejected a publish because that version is already there.
 * The server answers in plain text, e.g.
 * "Version 2.1.10 already exists. Increment the version number and try again."
 */
export function isVersionAlreadyExists(detail) {
  return /already exists/i.test(detail ?? "");
}

/** Build the multipart payload JSON, matching the Rust build_publish_payload. */
export function buildPublishPayload({ slug, displayName, version, changelog, ownerHandle, categories, topics }) {
  const payload = {
    slug,
    displayName: (displayName ?? "").trim(),
    version: (version ?? "").trim(),
    changelog: (changelog ?? "").trim(),
    acceptLicenseTerms: true,
    // dist-tag channel, not a display topic; ClawHub requires an array.
    tags: ["latest"],
  };

  const owner = (ownerHandle ?? "").trim().replace(/^@+/, "");
  if (owner) {
    payload.ownerHandle = owner;
  }
  if (Array.isArray(categories) && categories.length > 0) {
    payload.categories = categories;
  }
  if (Array.isArray(topics) && topics.length > 0) {
    payload.topics = topics;
  }
  return payload;
}

/**
 * Publish a skill directory to ClawHub via multipart. Returns
 * { ok, versionId, publicationStatus, externalUrl, version }.
 */
export async function publishSkill({ skillDir, token, slug, displayName, version, changelog, ownerHandle, categories = [], topics = [] }, { fetchImpl } = {}) {
  const trimmedToken = (token ?? "").trim();
  if (!trimmedToken) {
    throw new Error("尚未配置 ClawHub API token");
  }
  const cleanSlug = sanitizeSlug(slug);
  if (!cleanSlug) {
    throw new Error("slug 不能为空，且需包含字母或数字");
  }
  if (!(displayName ?? "").trim()) {
    throw new Error("展示名称不能为空");
  }
  if (!parseSemver(version)) {
    throw new Error(`版本号 "${version}" 不是合法的 semver（例如 1.0.0）`);
  }

  const files = collectSkillFiles(skillDir);
  const payload = buildPublishPayload({ slug: cleanSlug, displayName, version, changelog, ownerHandle, categories, topics });

  const form = new FormData();
  form.append("payload", JSON.stringify(payload));
  for (const file of files) {
    const bytes = readFileSync(file.absPath);
    form.append("files", new Blob([bytes], { type: file.contentType }), file.relPath);
  }

  const doFetch = fetchImpl ?? ((url, opts) => request(url, opts, PUBLISH_TIMEOUT_MS));
  const response = await doFetch(`${CLAWHUB_API_BASE}/skills`, {
    method: "POST",
    headers: { Authorization: `Bearer ${trimmedToken}`, "User-Agent": "skills-manager" },
    body: form,
  });

  if (!response.ok) {
    const detail = (await response.text().catch(() => "")).trim();
    if (!detail) {
      throw new Error(`发布失败，ClawHub 返回状态 ${response.status}`);
    }
    if (response.status === 401) {
      throw new Error(`ClawHub token 无效或无权发布: ${detail}`);
    }
    if (response.status === 429) {
      throw new Error(`发布过于频繁，请稍后重试: ${detail}`);
    }
    if (isVersionAlreadyExists(detail)) {
      // The version gate could not see this version — ClawHub's /versions
      // endpoint omits versions still pending their security scan — but the
      // server knows it exists. That is a no-op, not a failure, so flag it
      // for the caller instead of surfacing a red build.
      const error = new Error(`版本 ${(version ?? "").trim()} 已存在于 ClawHub: ${detail}`);
      error.versionAlreadyExists = true;
      throw error;
    }
    throw new Error(`发布失败: ${detail}`);
  }

  const parsed = await response.json().catch(() => ({}));

  let owner = (ownerHandle ?? "").trim().replace(/^@+/, "");
  if (!owner) {
    owner = (await verifyToken(trimmedToken, { fetchImpl }).catch(() => null))?.handle ?? "";
  }
  const externalUrl = owner ? `${CLAWHUB_SITE_ORIGIN}/${owner}/skills/${cleanSlug}` : undefined;

  return {
    ok: Boolean(parsed?.ok),
    versionId: parsed?.versionId,
    publicationStatus: parsed?.publicationStatus,
    externalUrl,
    version: (version ?? "").trim(),
  };
}
