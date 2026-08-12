use std::fs;
use std::path::Path;
use std::time::Duration;

use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::Deserialize;

use crate::models::publish::{
    CLAWHUB_CATEGORIES, MAX_CATEGORIES, MAX_PUBLISH_FILE_BYTES, MAX_PUBLISH_TOTAL_BYTES,
    MAX_TOPICS, MAX_TOPIC_LEN, RESERVED_TOPICS,
};
use crate::models::{
    ClawhubIdentity, PublishFile, PublishFileEntry, PublishPreview, PublishRequest, PublishResult,
    SkillPublishRecord,
};

const CLAWHUB_API_BASE: &str = "https://clawhub.ai/api/v1";
const CLAWHUB_SITE_ORIGIN: &str = "https://clawhub.ai";
/// 发布可能上传数十 MB，超时放宽到 120 秒。
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(120);
/// 只读查询（whoami / versions）用较短超时。
const QUERY_TIMEOUT: Duration = Duration::from_secs(20);

fn client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .user_agent("skills-manager")
        .timeout(timeout)
        .build()
        .map_err(|e| format!("无法创建 HTTP 客户端: {}", e))
}

// ===== 文件收集 =====

/// 路径中是否含有以 `.` 开头的片段。ClawHub 的 CLI 用同样的规则跳过
/// `.git`、`.DS_Store`、`.env` 等隐藏文件，避免把本地垃圾和密钥传上去。
fn has_dot_segment(rel_path: &str) -> bool {
    rel_path.split('/').any(|segment| segment.starts_with('.'))
}

/// 始终排除的目录名，与 ClawHub CLI 的默认忽略集一致。
fn is_excluded_dir(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | ".clawhub" | "__pycache__")
}

fn guess_content_type(rel_path: &str) -> String {
    let ext = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "json" => "application/json",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" => "text/typescript",
        "py" => "text/x-python",
        "sh" | "bash" => "application/x-sh",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    };
    mime.to_string()
}

/// 递归收集技能目录下要发布的文件。
///
/// 跳过隐藏路径与 `node_modules` 等目录，并逐个校验单文件上限；
/// 总大小上限由调用方在拿到完整列表后校验。
fn walk_publish_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<PublishFile>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(current).map_err(|e| format!("无法读取目录 {}: {}", current.display(), e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            if is_excluded_dir(&name) || name.starts_with('.') {
                continue;
            }
            walk_publish_files(root, &path, out)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");

        if has_dot_segment(&rel_path) {
            continue;
        }

        let metadata =
            fs::metadata(&path).map_err(|e| format!("无法读取文件信息 {}: {}", rel_path, e))?;
        if metadata.len() > MAX_PUBLISH_FILE_BYTES {
            return Err(format!("文件 \"{}\" 超过 10MB 单文件上限", rel_path));
        }

        let bytes = fs::read(&path).map_err(|e| format!("无法读取文件 {}: {}", rel_path, e))?;
        let content_type = guess_content_type(&rel_path);
        out.push(PublishFile {
            rel_path,
            bytes,
            content_type,
        });
    }

    Ok(())
}

/// 收集技能目录下的全部可发布文件，并校验总大小与必需的 SKILL.md。
pub fn collect_publish_files(root: &Path) -> Result<Vec<PublishFile>, String> {
    if !root.is_dir() {
        return Err(format!("技能目录不存在: {}", root.display()));
    }

    let mut files = Vec::new();
    walk_publish_files(root, root, &mut files)?;

    if files.is_empty() {
        return Err("技能目录为空，没有可发布的文件".to_string());
    }

    let has_skill_md = files
        .iter()
        .any(|file| file.rel_path.eq_ignore_ascii_case("SKILL.md"));
    if !has_skill_md {
        return Err("技能根目录缺少 SKILL.md，ClawHub 要求必须包含该文件".to_string());
    }

    let total: u64 = files.iter().map(|file| file.bytes.len() as u64).sum();
    if total > MAX_PUBLISH_TOTAL_BYTES {
        return Err("技能总大小超过 50MB 上限".to_string());
    }

    // 排序让预览与上传顺序稳定，便于用户核对。
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

// ===== slug / 元数据推导与校验 =====

/// 把任意名称规整成 ClawHub 可接受的 slug：小写、仅保留字母数字与连字符。
pub fn sanitize_slug(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// 话题归一化：小写、空白转连字符。ClawHub 用归一化后的形式做去重与保留词判定。
fn normalize_topic(input: &str) -> String {
    sanitize_slug(input)
}

/// 按 ClawHub 的规则校验分类与话题，尽量在本地拦截，避免上传后才失败。
pub fn validate_taxonomy(categories: &[String], topics: &[String]) -> Result<(), String> {
    if categories.len() > MAX_CATEGORIES {
        return Err(format!("最多只能选择 {} 个分类", MAX_CATEGORIES));
    }
    for category in categories {
        if !CLAWHUB_CATEGORIES.contains(&category.as_str()) {
            return Err(format!("未知的分类: {}", category));
        }
    }

    if topics.len() > MAX_TOPICS {
        return Err(format!("最多只能填写 {} 个话题", MAX_TOPICS));
    }
    for topic in topics {
        let normalized = normalize_topic(topic);
        if normalized.is_empty() {
            return Err("话题不能为空".to_string());
        }
        if topic.chars().count() > MAX_TOPIC_LEN {
            return Err(format!("话题 \"{}\" 超过 {} 个字符", topic, MAX_TOPIC_LEN));
        }
        if RESERVED_TOPICS.contains(&normalized.as_str()) {
            return Err(format!("话题 \"{}\" 是 ClawHub 保留词，无法使用", topic));
        }
    }

    Ok(())
}

/// 把 `1.2.3` 的 patch 位加一；解析失败时返回 None。
fn bump_patch(version: &str) -> Option<String> {
    let parsed = semver::Version::parse(version.trim()).ok()?;
    Some(format!(
        "{}.{}.{}",
        parsed.major,
        parsed.minor,
        parsed.patch + 1
    ))
}

// ===== ClawHub API =====

#[derive(Debug, Deserialize)]
struct WhoamiResponse {
    user: WhoamiUser,
}

#[derive(Debug, Deserialize)]
struct WhoamiUser {
    handle: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    image: Option<String>,
}

/// 校验 token 并返回身份信息。token 为空时直接报错，不发请求。
pub async fn verify_token(token: &str) -> Result<ClawhubIdentity, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("尚未配置 ClawHub API token".to_string());
    }

    let response = client(QUERY_TIMEOUT)?
        .get(format!("{}/whoami", CLAWHUB_API_BASE))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("连接 ClawHub 失败: {}", e))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("ClawHub token 无效或已过期".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("ClawHub 返回错误状态: {}", response.status()));
    }

    let parsed: WhoamiResponse = response
        .json()
        .await
        .map_err(|e| format!("解析 ClawHub 响应失败: {}", e))?;

    Ok(ClawhubIdentity {
        handle: parsed.user.handle,
        display_name: parsed.user.display_name,
        image: parsed.user.image,
    })
}

#[derive(Debug, Deserialize)]
struct VersionsResponse {
    #[serde(default)]
    versions: Vec<VersionItem>,
}

#[derive(Debug, Deserialize)]
struct VersionItem {
    version: Option<String>,
}

/// 远端版本查询结果。必须把"确实没发布过"和"查不到"区分开：
/// 前者可以安全地从 1.0.0 起步，后者若也按 1.0.0 处理，
/// 会在断网时把一个已发布的技能重复发布成新技能。
enum VersionLookup {
    /// 技能存在，最新版本如下。
    Found(String),
    /// 技能确实不存在（404）。
    NotPublished,
    /// 查询失败，结果不可信。
    Failed,
}

/// 查询远端某 slug 的最新版本。
async fn fetch_latest_version(slug: &str, owner: Option<&str>) -> VersionLookup {
    let client = match client(QUERY_TIMEOUT) {
        Ok(client) => client,
        Err(_) => return VersionLookup::Failed,
    };

    let mut request = client.get(format!("{}/skills/{}/versions", CLAWHUB_API_BASE, slug));
    if let Some(owner) = owner.filter(|value| !value.trim().is_empty()) {
        request = request.query(&[("owner", owner)]);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return VersionLookup::Failed,
    };

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return VersionLookup::NotPublished;
    }
    if !response.status().is_success() {
        return VersionLookup::Failed;
    }

    let parsed: VersionsResponse = match response.json().await {
        Ok(value) => value,
        Err(_) => return VersionLookup::Failed,
    };

    parsed
        .versions
        .into_iter()
        .filter_map(|item| item.version)
        .filter_map(|value| semver::Version::parse(value.trim()).ok())
        .max()
        .map(|value| VersionLookup::Found(value.to_string()))
        // 端点返回成功但没有任何版本，等同于未发布。
        .unwrap_or(VersionLookup::NotPublished)
}

/// 在两个 semver 中取较大者；解析失败的一方被忽略。
fn max_version(a: Option<&str>, b: Option<&str>) -> Option<String> {
    let parse =
        |value: Option<&str>| value.and_then(|value| semver::Version::parse(value.trim()).ok());
    match (parse(a), parse(b)) {
        (Some(left), Some(right)) => Some(if left >= right { left } else { right }.to_string()),
        (Some(left), None) => Some(left.to_string()),
        (None, Some(right)) => Some(right.to_string()),
        (None, None) => None,
    }
}

/// 组装发布前的预览信息：文件清单 + 建议版本号 + 默认元数据。
///
/// `record` 是上次成功发布留下的本地记录。有记录时一切以它为准 ——
/// slug 和 owner 沿用上次的，否则用户改过 slug 或发布到组织名下后，
/// 按目录名重新推导会查错对象，进而误判成"未发布"并重复创建技能。
pub async fn build_preview(
    skill_dir: &Path,
    skill_name: &str,
    token: Option<&str>,
    record: Option<&SkillPublishRecord>,
) -> Result<PublishPreview, String> {
    let files = collect_publish_files(skill_dir)?;
    let total_bytes: u64 = files.iter().map(|file| file.bytes.len() as u64).sum();

    let suggested_slug = match record {
        Some(record) => record.slug.clone(),
        None => {
            let dir_name = skill_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(skill_name);
            let from_dir = sanitize_slug(dir_name);
            if from_dir.is_empty() {
                sanitize_slug(skill_name)
            } else {
                from_dir
            }
        }
    };

    // 查询归属：优先沿用上次发布的 owner，否则用 token 对应的账号。
    let suggested_owner_handle = record.and_then(|record| record.owner_handle.clone());
    let lookup_owner = match &suggested_owner_handle {
        Some(owner) => Some(owner.clone()),
        None => match token.filter(|value| !value.trim().is_empty()) {
            Some(token) => verify_token(token).await.ok().and_then(|id| id.handle),
            None => None,
        },
    };

    let lookup = if suggested_slug.is_empty() {
        VersionLookup::NotPublished
    } else {
        fetch_latest_version(&suggested_slug, lookup_owner.as_deref()).await
    };

    let (latest_version, version_lookup_failed) = match lookup {
        VersionLookup::Found(version) => (Some(version), false),
        VersionLookup::NotPublished => (None, false),
        VersionLookup::Failed => (None, true),
    };

    // 版本基线取"远端最新"与"本地记录"的较大者：远端查询失败时仍能
    // 基于本地记录递增，不会退回 1.0.0 而把已发布技能覆盖成新版本。
    let baseline = max_version(
        latest_version.as_deref(),
        record.map(|record| record.version.as_str()),
    );
    let suggested_version = baseline
        .as_deref()
        .and_then(bump_patch)
        .unwrap_or_else(|| "1.0.0".to_string());

    let warning = match (&latest_version, version_lookup_failed) {
        (_, true) => Some(format!(
            "无法查询 ClawHub 上 \"{}\" 的版本（网络或服务异常）。建议版本号可能不准确，发布前请自行确认。",
            suggested_slug
        )),
        (Some(version), false) => Some(format!(
            "ClawHub 上已存在 slug \"{}\"，最新版本 {}。继续发布会创建新版本。",
            suggested_slug, version
        )),
        (None, false) => None,
    };

    Ok(PublishPreview {
        files: files
            .into_iter()
            .map(|file| PublishFileEntry {
                rel_path: file.rel_path,
                size: file.bytes.len() as u64,
            })
            .collect(),
        total_bytes,
        suggested_slug,
        suggested_display_name: skill_name.to_string(),
        suggested_owner_handle,
        latest_version,
        suggested_version,
        existing_record: record.cloned(),
        version_lookup_failed,
        warning,
    })
}

#[derive(Debug, Deserialize)]
struct PublishApiResponse {
    #[serde(default)]
    ok: bool,
    #[serde(rename = "versionId", default)]
    version_id: Option<String>,
    #[serde(rename = "publicationStatus", default)]
    publication_status: Option<String>,
}

/// 构造发布 payload。
///
/// 字段名与取舍完全由 ClawHub 的 `parseMultipartPublish` + `CliPublishRequestSchema`
/// 决定，抽成纯函数是为了能在不发网络请求的前提下测试字段完整性。
///
/// `tags` 是发布通道标签（dist-tag，类似 npm 的 latest/beta），**不是**展示用的话题 ——
/// 展示话题走 `topics`。服务端对 tags 用的是直接赋值而非条件展开，缺省会变成
/// `tags: undefined` 并被 schema 判成"不是数组"而报 `tags: an array`，
/// 所以必须显式带上，默认值与官方 CLI 一致取 `["latest"]`。
fn build_publish_payload(request: &PublishRequest, slug: &str) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "slug": slug,
        "displayName": request.display_name.trim(),
        "version": request.version.trim(),
        "changelog": request.changelog.trim(),
        "acceptLicenseTerms": true,
        "tags": ["latest"],
    });

    if let Some(owner) = request
        .owner_handle
        .as_ref()
        .map(|value| value.trim().trim_start_matches('@'))
        .filter(|value| !value.is_empty())
    {
        payload["ownerHandle"] = serde_json::Value::String(owner.to_string());
    }
    if !request.categories.is_empty() {
        payload["categories"] = serde_json::json!(request.categories);
    }
    if !request.topics.is_empty() {
        payload["topics"] = serde_json::json!(request.topics);
    }

    payload
}

/// 以 multipart 方式发布技能到 ClawHub。
///
/// 走 multipart 而非 JSON：JSON 路径要求每个文件预先通过上传接口换取
/// `uploadTicket`，multipart 则一次请求完成，服务端自行落盘并算 sha256。
pub async fn publish_skill(
    request: &PublishRequest,
    skill_dir: &Path,
    token: &str,
) -> Result<PublishResult, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("尚未配置 ClawHub API token".to_string());
    }
    if !request.accept_license_terms {
        return Err("必须接受 MIT-0 许可条款才能发布到 ClawHub".to_string());
    }

    let slug = sanitize_slug(&request.slug);
    if slug.is_empty() {
        return Err("slug 不能为空，且需包含字母或数字".to_string());
    }
    if request.display_name.trim().is_empty() {
        return Err("展示名称不能为空".to_string());
    }
    if semver::Version::parse(request.version.trim()).is_err() {
        return Err(format!(
            "版本号 \"{}\" 不是合法的 semver（例如 1.0.0）",
            request.version
        ));
    }
    validate_taxonomy(&request.categories, &request.topics)?;

    let files = collect_publish_files(skill_dir)?;

    let payload = build_publish_payload(request, &slug);
    let payload_text =
        serde_json::to_string(&payload).map_err(|e| format!("构造发布数据失败: {}", e))?;

    let mut form = Form::new().text("payload", payload_text);
    for file in files {
        let part = Part::bytes(file.bytes)
            .file_name(file.rel_path.clone())
            .mime_str(&file.content_type)
            .map_err(|e| format!("构造上传文件 {} 失败: {}", file.rel_path, e))?;
        form = form.part("files", part);
    }

    let response = client(PUBLISH_TIMEOUT)?
        .post(format!("{}/skills", CLAWHUB_API_BASE))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("上传到 ClawHub 失败: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        // 发布端点用纯文本返回错误原因，直接透传给用户最有用。
        let body = response.text().await.unwrap_or_default();
        let detail = body.trim();
        return Err(if detail.is_empty() {
            format!("发布失败，ClawHub 返回状态 {}", status)
        } else if status == reqwest::StatusCode::UNAUTHORIZED {
            format!("ClawHub token 无效或无权发布: {}", detail)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            format!("发布过于频繁，请稍后重试: {}", detail)
        } else {
            format!("发布失败: {}", detail)
        });
    }

    let parsed: PublishApiResponse = response
        .json()
        .await
        .map_err(|e| format!("解析发布结果失败: {}", e))?;

    let owner_for_url = request
        .owner_handle
        .as_ref()
        .map(|value| value.trim().trim_start_matches('@').to_string())
        .filter(|value| !value.is_empty());
    let owner_for_url = match owner_for_url {
        Some(owner) => Some(owner),
        None => verify_token(token).await.ok().and_then(|id| id.handle),
    };
    let external_url =
        owner_for_url.map(|owner| format!("{}/{}/skills/{}", CLAWHUB_SITE_ORIGIN, owner, slug));

    Ok(PublishResult {
        ok: parsed.ok,
        version_id: parsed.version_id,
        publication_status: parsed.publication_status,
        external_url,
        version: request.version.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn sanitize_slug_normalizes_arbitrary_names() {
        assert_eq!(sanitize_slug("My Skill"), "my-skill");
        assert_eq!(
            sanitize_slug("  Git Worktree Cleanup "),
            "git-worktree-cleanup"
        );
        assert_eq!(sanitize_slug("a__b--c"), "a-b-c");
        assert_eq!(sanitize_slug("---"), "");
        assert_eq!(sanitize_slug("PDF2Text"), "pdf2text");
    }

    #[test]
    fn bump_patch_increments_last_component() {
        assert_eq!(bump_patch("1.0.0").as_deref(), Some("1.0.1"));
        assert_eq!(bump_patch("2.3.9").as_deref(), Some("2.3.10"));
        assert_eq!(bump_patch("not-semver"), None);
    }

    /// 远端查询失败时要能靠本地记录继续递增，而不是退回 1.0.0
    /// 把已发布的技能重复发布一遍。
    #[test]
    fn max_version_prefers_the_higher_known_version() {
        assert_eq!(
            max_version(Some("1.2.3"), Some("1.3.0")).as_deref(),
            Some("1.3.0")
        );
        assert_eq!(
            max_version(Some("2.0.0"), Some("1.9.9")).as_deref(),
            Some("2.0.0")
        );
        // 远端查不到时退回本地记录。
        assert_eq!(max_version(None, Some("1.4.2")).as_deref(), Some("1.4.2"));
        // 没有本地记录时用远端。
        assert_eq!(max_version(Some("1.0.5"), None).as_deref(), Some("1.0.5"));
        assert_eq!(max_version(None, None), None);
        // 无法解析的一方被忽略，而不是让整体退化。
        assert_eq!(
            max_version(Some("garbage"), Some("1.1.0")).as_deref(),
            Some("1.1.0")
        );
    }

    #[test]
    fn has_dot_segment_detects_hidden_paths() {
        assert!(has_dot_segment(".env"));
        assert!(has_dot_segment("scripts/.secret/key.txt"));
        assert!(!has_dot_segment("scripts/run.sh"));
        // 文件名中间的点不算隐藏路径。
        assert!(!has_dot_segment("SKILL.md"));
    }

    #[test]
    fn collect_publish_files_skips_hidden_and_excluded_paths() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        fs::write(root.join("SKILL.md"), b"---\nname: demo\n---\n").unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/run.sh"), b"echo hi").unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), b"x").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), b"x").unwrap();
        fs::write(root.join(".env"), b"SECRET=1").unwrap();

        let files = collect_publish_files(root).expect("collect");
        let paths: Vec<_> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(paths, vec!["SKILL.md", "scripts/run.sh"]);
    }

    #[test]
    fn collect_publish_files_requires_skill_md() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("README.md"), b"nope").unwrap();
        let err = collect_publish_files(dir.path()).unwrap_err();
        assert!(err.contains("SKILL.md"), "unexpected error: {}", err);
    }

    #[test]
    fn validate_taxonomy_enforces_clawhub_rules() {
        assert!(validate_taxonomy(&["development".into()], &["git".into()]).is_ok());

        let err = validate_taxonomy(&["Development".into()], &[]).unwrap_err();
        assert!(err.contains("未知的分类"), "{}", err);

        let too_many = vec![
            "development".to_string(),
            "operations".to_string(),
            "security".to_string(),
            "finance".to_string(),
        ];
        assert!(validate_taxonomy(&too_many, &[]).is_err());

        let reserved = validate_taxonomy(&[], &["Official".into()]).unwrap_err();
        assert!(reserved.contains("保留词"), "{}", reserved);

        let long_topic = "x".repeat(MAX_TOPIC_LEN + 1);
        assert!(validate_taxonomy(&[], &[long_topic]).is_err());
    }

    #[test]
    fn guess_content_type_covers_common_skill_files() {
        assert_eq!(guess_content_type("SKILL.md"), "text/markdown");
        assert_eq!(guess_content_type("scripts/run.sh"), "application/x-sh");
        assert_eq!(guess_content_type("data/x.bin"), "application/octet-stream");
    }

    fn sample_request() -> PublishRequest {
        PublishRequest {
            instance_id: "global:demo".to_string(),
            slug: "demo".to_string(),
            display_name: "  Demo Skill  ".to_string(),
            version: " 1.2.3 ".to_string(),
            changelog: " first release ".to_string(),
            categories: Vec::new(),
            topics: Vec::new(),
            owner_handle: None,
            accept_license_terms: true,
        }
    }

    /// tags 缺省会让 ClawHub 报 `Publish payload: tags: an array`，
    /// 因为服务端把它当必填数组处理。这条测试守住该字段。
    #[test]
    fn build_publish_payload_always_includes_tags_array() {
        let payload = build_publish_payload(&sample_request(), "demo");
        assert_eq!(payload["tags"], serde_json::json!(["latest"]));
        assert!(payload["tags"].is_array());
    }

    #[test]
    fn build_publish_payload_trims_fields_and_accepts_license() {
        let payload = build_publish_payload(&sample_request(), "demo");
        assert_eq!(payload["slug"], "demo");
        assert_eq!(payload["displayName"], "Demo Skill");
        assert_eq!(payload["version"], "1.2.3");
        assert_eq!(payload["changelog"], "first release");
        assert_eq!(payload["acceptLicenseTerms"], true);
    }

    #[test]
    fn build_publish_payload_omits_empty_optional_fields() {
        let payload = build_publish_payload(&sample_request(), "demo");
        // 可选字段必须整个缺席，而不是显式的 null —— 服务端 schema 对
        // "键存在但值为空" 的处理与"键缺席"不同。
        assert!(payload.get("ownerHandle").is_none());
        assert!(payload.get("categories").is_none());
        assert!(payload.get("topics").is_none());
    }

    #[test]
    fn build_publish_payload_includes_owner_categories_and_topics() {
        let request = PublishRequest {
            categories: vec!["development".to_string()],
            topics: vec!["git".to_string(), "worktree".to_string()],
            owner_handle: Some("@my-org".to_string()),
            ..sample_request()
        };
        let payload = build_publish_payload(&request, "demo");
        // ownerHandle 需剥掉前导 @。
        assert_eq!(payload["ownerHandle"], "my-org");
        assert_eq!(payload["categories"], serde_json::json!(["development"]));
        assert_eq!(payload["topics"], serde_json::json!(["git", "worktree"]));
    }
}
