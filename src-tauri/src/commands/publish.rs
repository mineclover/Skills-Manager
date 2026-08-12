use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::publish::CLAWHUB_CATEGORIES;
use crate::models::{
    ClawhubIdentity, PublishPreview, PublishRequest, PublishResult, SkillMetadata,
    SkillPublishRecord,
};
use crate::services::{publish, AppCache, ConfigManager, ScannerService};
use tauri::State;

/// 从配置里取 ClawHub token，未配置时返回可读的错误。
fn require_token() -> Result<String, String> {
    let config = ConfigManager::new().load()?;
    config
        .preferences
        .and_then(|prefs| prefs.clawhub_token)
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "尚未配置 ClawHub API token，请前往设置填写".to_string())
}

/// 取 token，但允许缺失（预览时用来决定是否查询远端版本）。
fn optional_token() -> Option<String> {
    ConfigManager::new()
        .load()
        .ok()
        .and_then(|config| config.preferences)
        .and_then(|prefs| prefs.clawhub_token)
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// 按 instance_id 定位本地技能，返回 (目录, 展示名)。
fn resolve_skill(instance_id: &str, cache: &State<AppCache>) -> Result<(PathBuf, String), String> {
    let config = ConfigManager::new().load()?;
    let skills = match cache.get_skills() {
        Some(skills) => skills,
        None => {
            let fresh = ScannerService::scan_scoped_skills(&config)?;
            cache.set_skills(fresh.clone());
            fresh
        }
    };

    skills
        .into_iter()
        .find(|skill| skill.instance_id == instance_id)
        .map(|skill| (skill.path, skill.name))
        .ok_or_else(|| format!("找不到技能: {}", instance_id))
}

/// 返回 ClawHub 支持的分类枚举，供前端渲染选择器。
#[tauri::command]
pub fn get_clawhub_categories() -> Vec<String> {
    CLAWHUB_CATEGORIES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

/// 校验当前配置的 ClawHub token，并返回账号信息。
#[tauri::command]
pub async fn verify_clawhub_token(token: Option<String>) -> Result<ClawhubIdentity, String> {
    // 允许传入待保存的 token 做"测试连接"，未传则用已保存的。
    let token = match token.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => value,
        _ => require_token()?,
    };
    publish::verify_token(&token).await
}

/// 发布前预检：列出将上传的文件、总大小，并推导建议 slug 与版本号。
#[tauri::command]
pub async fn preview_clawhub_publish(
    instance_id: String,
    cache: State<'_, AppCache>,
) -> Result<PublishPreview, String> {
    let (skill_dir, skill_name) = resolve_skill(&instance_id, &cache)?;
    let token = optional_token();
    let record = ConfigManager::new()
        .load()
        .ok()
        .and_then(|config| config.skill_metadata.get(&instance_id).cloned())
        .and_then(|metadata| metadata.publish);
    publish::build_preview(&skill_dir, &skill_name, token.as_deref(), record.as_ref()).await
}

/// 把本地技能发布到 ClawHub，成功后把 slug/owner/版本写回本地配置。
#[tauri::command]
pub async fn publish_skill_to_clawhub(
    request: PublishRequest,
    cache: State<'_, AppCache>,
) -> Result<PublishResult, String> {
    let token = require_token()?;
    let (skill_dir, _) = resolve_skill(&request.instance_id, &cache)?;
    let result = publish::publish_skill(&request, &skill_dir, &token).await?;
    // 回写失败不该让用户以为发布失败 —— 发布已经成功了，
    // 只是本地标识会缺失，下次预览会退回按目录名推导。
    if let Err(error) = record_publish(&request, &result) {
        eprintln!("发布成功但写入本地记录失败: {}", error);
    }
    Ok(result)
}

/// 把本次发布的定位信息持久化到 skill_metadata，
/// 下次发布据此沿用同一个 slug/owner。
fn record_publish(request: &PublishRequest, result: &PublishResult) -> Result<(), String> {
    let manager = ConfigManager::new();
    let mut config = manager.load()?;

    let owner_handle = request
        .owner_handle
        .as_ref()
        .map(|value| value.trim().trim_start_matches('@').to_string())
        .filter(|value| !value.is_empty());

    let entry = config
        .skill_metadata
        .entry(request.instance_id.clone())
        .or_insert_with(SkillMetadata::default);
    entry.publish = Some(SkillPublishRecord {
        slug: publish::sanitize_slug(&request.slug),
        owner_handle,
        version: result.version.clone(),
        published_at: now_timestamp(),
        publication_status: result.publication_status.clone(),
        external_url: result.external_url.clone(),
    });

    manager.save(&config)
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
