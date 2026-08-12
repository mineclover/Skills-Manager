import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MODAL_LAYER_Z_INDEX, MODAL_OVERLAY_COLOR } from "@/constants/modal";
import type { PublishPreview, PublishResult, Skill } from "@/types";
import type { TranslationPath } from "@/i18n";

interface PublishToClawhubDialogProps {
  open: boolean;
  skill: Skill | null;
  onClose: () => void;
  onPublished: (result: PublishResult) => void;
  t: (key: TranslationPath) => string;
}

/** 单个话题最长 48 字符，与后端 MAX_TOPIC_LEN 保持一致。 */
const MAX_TOPIC_LEN = 48;
const MAX_CATEGORIES = 3;
const MAX_TOPICS = 5;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 把逗号分隔的输入拆成去空的数组。 */
function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

/** 后端存的是秒级时间戳。 */
function formatPublishedAt(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString();
}

const labelStyle: React.CSSProperties = {
  fontSize: "12px",
  fontWeight: 600,
  color: "var(--foreground)",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "7px 10px",
  fontSize: "13px",
  color: "var(--foreground)",
  backgroundColor: "var(--background)",
  border: "1px solid var(--border)",
  borderRadius: "6px",
  outline: "none",
};

const hintStyle: React.CSSProperties = {
  fontSize: "11px",
  color: "var(--muted-foreground)",
};

const fieldStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "5px",
};

export function PublishToClawhubDialog({
  open,
  skill,
  onClose,
  onPublished,
  t,
}: PublishToClawhubDialogProps) {
  const [preview, setPreview] = useState<PublishPreview | null>(null);
  const [categoryOptions, setCategoryOptions] = useState<string[]>([]);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [publishing, setPublishing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [slug, setSlug] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [version, setVersion] = useState("");
  const [changelog, setChangelog] = useState("");
  const [categories, setCategories] = useState<string[]>([]);
  const [topicsInput, setTopicsInput] = useState("");
  const [ownerHandle, setOwnerHandle] = useState("");
  const [acceptLicense, setAcceptLicense] = useState(false);

  // 每次打开都重新预检：文件可能已改动，远端版本也可能变了。
  useEffect(() => {
    if (!open || !skill) {
      return;
    }

    let cancelled = false;
    setLoadingPreview(true);
    setError(null);
    setPreview(null);
    setAcceptLicense(false);
    setChangelog("");
    setCategories([]);
    setTopicsInput("");
    setOwnerHandle("");

    const load = async () => {
      try {
        const [result, options] = await Promise.all([
          invoke<PublishPreview>("preview_clawhub_publish", {
            instanceId: skill.instance_id,
          }),
          invoke<string[]>("get_clawhub_categories"),
        ]);
        if (cancelled) return;
        setPreview(result);
        setCategoryOptions(options);
        setSlug(result.suggested_slug);
        setDisplayName(result.suggested_display_name);
        setVersion(result.suggested_version);
        // 沿用上次发布的归属账号，避免更新时误发到别的 owner 下。
        setOwnerHandle(result.suggested_owner_handle ?? "");
      } catch (err) {
        if (cancelled) return;
        setError(String(err));
      } finally {
        if (!cancelled) setLoadingPreview(false);
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [open, skill]);

  const topics = useMemo(() => parseCsv(topicsInput), [topicsInput]);

  const validationError = useMemo(() => {
    if (!slug.trim()) return t("publish.errorSlugRequired");
    if (!displayName.trim()) return t("publish.errorNameRequired");
    if (!/^\d+\.\d+\.\d+/.test(version.trim())) return t("publish.errorVersionInvalid");
    if (categories.length > MAX_CATEGORIES) return t("publish.errorTooManyCategories");
    if (topics.length > MAX_TOPICS) return t("publish.errorTooManyTopics");
    if (topics.some((topic) => topic.length > MAX_TOPIC_LEN)) {
      return t("publish.errorTopicTooLong");
    }
    return null;
  }, [slug, displayName, version, categories, topics, t]);

  if (!open || !skill) {
    return null;
  }

  const toggleCategory = (category: string) => {
    setCategories((current) => {
      if (current.includes(category)) {
        return current.filter((entry) => entry !== category);
      }
      if (current.length >= MAX_CATEGORIES) {
        return current;
      }
      return [...current, category];
    });
  };

  const handlePublish = async () => {
    if (validationError || !acceptLicense) {
      return;
    }
    setPublishing(true);
    setError(null);
    try {
      const result = await invoke<PublishResult>("publish_skill_to_clawhub", {
        request: {
          instance_id: skill.instance_id,
          slug: slug.trim(),
          display_name: displayName.trim(),
          version: version.trim(),
          changelog: changelog.trim(),
          categories,
          topics,
          owner_handle: ownerHandle.trim() || null,
          accept_license_terms: acceptLicense,
        },
      });
      onPublished(result);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setPublishing(false);
    }
  };

  const busy = publishing || loadingPreview;
  const canPublish = !busy && !validationError && acceptLicense && preview !== null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        backgroundColor: MODAL_OVERLAY_COLOR,
        zIndex: MODAL_LAYER_Z_INDEX,
      }}
      onClick={busy ? undefined : onClose}
    >
      <div
        style={{
          width: "min(660px, calc(100vw - 48px))",
          maxHeight: "calc(100vh - 72px)",
          backgroundColor: "var(--background)",
          borderRadius: "12px",
          border: "1px solid var(--border)",
          boxShadow: "0 16px 48px rgba(0,0,0,0.18)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          style={{
            padding: "18px 22px",
            borderBottom: "1px solid var(--border)",
            display: "flex",
            flexDirection: "column",
            gap: "4px",
          }}
        >
          <div style={{ fontSize: "15px", fontWeight: 600, color: "var(--foreground)" }}>
            {t("publish.title")}
          </div>
          <div style={{ fontSize: "13px", color: "var(--muted-foreground)" }}>
            {t("publish.subtitle").replace("{name}", skill.name)}
          </div>
        </div>

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: "auto",
            padding: "16px 22px",
            display: "flex",
            flexDirection: "column",
            gap: "14px",
          }}
        >
          {loadingPreview && (
            <div style={{ fontSize: "13px", color: "var(--muted-foreground)" }}>
              {t("publish.loadingPreview")}
            </div>
          )}

          {preview?.warning && (
            <div
              style={{
                padding: "10px 12px",
                borderRadius: "8px",
                fontSize: "12px",
                color: "var(--foreground)",
                backgroundColor: preview.version_lookup_failed
                  ? "var(--color-error-bg)"
                  : "var(--color-warning-bg, var(--secondary))",
                border: preview.version_lookup_failed
                  ? "1px solid var(--destructive)"
                  : "1px solid var(--border)",
              }}
            >
              {preview.warning}
            </div>
          )}

          {preview?.existing_record && (
            <div
              style={{
                padding: "10px 12px",
                borderRadius: "8px",
                fontSize: "12px",
                color: "var(--muted-foreground)",
                backgroundColor: "var(--secondary)",
                border: "1px solid var(--border)",
                display: "flex",
                flexDirection: "column",
                gap: "3px",
              }}
            >
              <span style={{ fontWeight: 600, color: "var(--foreground)" }}>
                {t("publish.lastPublished")
                  .replace("{slug}", preview.existing_record.slug)
                  .replace("{version}", preview.existing_record.version)}
              </span>
              <span>
                {formatPublishedAt(preview.existing_record.published_at)}
                {preview.existing_record.owner_handle
                  ? ` · @${preview.existing_record.owner_handle}`
                  : ""}
              </span>
            </div>
          )}

          {preview && (
            <>
              <div style={{ display: "flex", gap: "12px" }}>
                <div style={{ ...fieldStyle, flex: 1 }}>
                  <span style={labelStyle}>{t("publish.slug")}</span>
                  <input
                    style={inputStyle}
                    value={slug}
                    disabled={busy}
                    onChange={(e) => setSlug(e.target.value)}
                  />
                  <span style={hintStyle}>{t("publish.slugHint")}</span>
                </div>
                <div style={{ ...fieldStyle, flex: 1 }}>
                  <span style={labelStyle}>{t("publish.displayName")}</span>
                  <input
                    style={inputStyle}
                    value={displayName}
                    disabled={busy}
                    onChange={(e) => setDisplayName(e.target.value)}
                  />
                </div>
              </div>

              <div style={{ display: "flex", gap: "12px" }}>
                <div style={{ ...fieldStyle, flex: 1 }}>
                  <span style={labelStyle}>{t("publish.version")}</span>
                  <input
                    style={inputStyle}
                    value={version}
                    disabled={busy}
                    onChange={(e) => setVersion(e.target.value)}
                  />
                  <span style={hintStyle}>
                    {preview.latest_version
                      ? t("publish.versionHintExisting").replace(
                          "{version}",
                          preview.latest_version,
                        )
                      : t("publish.versionHintNew")}
                  </span>
                </div>
                <div style={{ ...fieldStyle, flex: 1 }}>
                  <span style={labelStyle}>{t("publish.ownerHandle")}</span>
                  <input
                    style={inputStyle}
                    value={ownerHandle}
                    disabled={busy}
                    placeholder={t("publish.ownerHandlePlaceholder")}
                    onChange={(e) => setOwnerHandle(e.target.value)}
                  />
                  <span style={hintStyle}>{t("publish.ownerHandleHint")}</span>
                </div>
              </div>

              <div style={fieldStyle}>
                <span style={labelStyle}>{t("publish.changelog")}</span>
                <textarea
                  style={{ ...inputStyle, minHeight: "60px", resize: "vertical" }}
                  value={changelog}
                  disabled={busy}
                  placeholder={t("publish.changelogPlaceholder")}
                  onChange={(e) => setChangelog(e.target.value)}
                />
              </div>

              <div style={fieldStyle}>
                <span style={labelStyle}>
                  {t("publish.categories")} ({categories.length}/{MAX_CATEGORIES})
                </span>
                <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
                  {categoryOptions.map((category) => {
                    const selected = categories.includes(category);
                    const disabled =
                      busy || (!selected && categories.length >= MAX_CATEGORIES);
                    return (
                      <button
                        key={category}
                        type="button"
                        disabled={disabled}
                        onClick={() => toggleCategory(category)}
                        style={{
                          padding: "4px 10px",
                          fontSize: "12px",
                          fontWeight: 500,
                          color: selected
                            ? "var(--primary-foreground)"
                            : "var(--foreground)",
                          backgroundColor: selected ? "var(--primary)" : "var(--background)",
                          border: selected
                            ? "1px solid var(--primary)"
                            : "1px solid var(--border)",
                          borderRadius: "999px",
                          cursor: disabled ? "not-allowed" : "pointer",
                          opacity: disabled ? 0.5 : 1,
                        }}
                      >
                        {category}
                      </button>
                    );
                  })}
                </div>
                <span style={hintStyle}>{t("publish.categoriesHint")}</span>
              </div>

              <div style={fieldStyle}>
                <span style={labelStyle}>
                  {t("publish.topics")} ({topics.length}/{MAX_TOPICS})
                </span>
                <input
                  style={inputStyle}
                  value={topicsInput}
                  disabled={busy}
                  placeholder={t("publish.topicsPlaceholder")}
                  onChange={(e) => setTopicsInput(e.target.value)}
                />
                <span style={hintStyle}>{t("publish.topicsHint")}</span>
              </div>

              <div style={fieldStyle}>
                <span style={labelStyle}>
                  {t("publish.files")
                    .replace("{count}", String(preview.files.length))
                    .replace("{size}", formatBytes(preview.total_bytes))}
                </span>
                <div
                  style={{
                    maxHeight: "120px",
                    overflowY: "auto",
                    padding: "8px 10px",
                    borderRadius: "6px",
                    border: "1px solid var(--border)",
                    backgroundColor: "var(--secondary)",
                    display: "flex",
                    flexDirection: "column",
                    gap: "3px",
                  }}
                >
                  {preview.files.map((file) => (
                    <div
                      key={file.rel_path}
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        gap: "12px",
                        fontSize: "11px",
                        color: "var(--muted-foreground)",
                      }}
                    >
                      <span style={{ wordBreak: "break-all" }}>{file.rel_path}</span>
                      <span style={{ flexShrink: 0 }}>{formatBytes(file.size)}</span>
                    </div>
                  ))}
                </div>
              </div>

              {/* MIT-0 是 ClawHub 服务端的硬性要求，必须由用户显式勾选，
                  不能由程序代为接受。 */}
              <label
                style={{
                  display: "flex",
                  alignItems: "flex-start",
                  gap: "8px",
                  padding: "10px 12px",
                  borderRadius: "8px",
                  border: "1px solid var(--border)",
                  backgroundColor: "var(--secondary)",
                  fontSize: "12px",
                  color: "var(--foreground)",
                  cursor: busy ? "not-allowed" : "pointer",
                }}
              >
                <input
                  type="checkbox"
                  checked={acceptLicense}
                  disabled={busy}
                  onChange={(e) => setAcceptLicense(e.target.checked)}
                  style={{ marginTop: "2px" }}
                />
                <span>{t("publish.licenseNotice")}</span>
              </label>
            </>
          )}

          {(error || validationError) && (
            <div
              style={{
                padding: "10px 12px",
                borderRadius: "8px",
                fontSize: "12px",
                color: "var(--destructive)",
                backgroundColor: "var(--color-error-bg)",
                border: "1px solid var(--destructive)",
                wordBreak: "break-word",
              }}
            >
              {error ?? validationError}
            </div>
          )}
        </div>

        <div
          style={{
            padding: "14px 22px",
            borderTop: "1px solid var(--border)",
            display: "flex",
            justifyContent: "flex-end",
            gap: "8px",
          }}
        >
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            style={{
              padding: "8px 16px",
              fontSize: "13px",
              fontWeight: 500,
              color: "var(--foreground)",
              backgroundColor: "var(--background)",
              border: "1px solid var(--border)",
              borderRadius: "8px",
              cursor: busy ? "not-allowed" : "pointer",
            }}
          >
            {t("publish.cancel")}
          </button>
          <button
            type="button"
            onClick={handlePublish}
            disabled={!canPublish}
            style={{
              padding: "8px 16px",
              fontSize: "13px",
              fontWeight: 500,
              color: "var(--primary-foreground)",
              backgroundColor: "var(--primary)",
              border: "1px solid var(--primary)",
              borderRadius: "8px",
              cursor: canPublish ? "pointer" : "not-allowed",
              opacity: canPublish ? 1 : 0.6,
            }}
          >
            {publishing ? t("publish.publishing") : t("publish.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
