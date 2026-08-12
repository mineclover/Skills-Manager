use serde::{Deserialize, Serialize};

use crate::models::config::SkillPublishRecord;

/// ClawHub 单文件大小上限（服务端 MAX_PUBLISH_FILE_BYTES）。
pub const MAX_PUBLISH_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// ClawHub 单次发布总大小上限（服务端 MAX_PUBLISH_TOTAL_BYTES）。
pub const MAX_PUBLISH_TOTAL_BYTES: u64 = 50 * 1024 * 1024;
/// 一个技能最多携带的分类数。
pub const MAX_CATEGORIES: usize = 3;
/// 一个技能最多携带的话题数。
pub const MAX_TOPICS: usize = 5;

/// ClawHub 浏览页的固定分类枚举。传入枚举外的值会导致发布失败，
/// 因此前端只能从这个列表里选。
pub const CLAWHUB_CATEGORIES: &[&str] = &[
    "integrations",
    "automation",
    "research",
    "development",
    "productivity",
    "communication",
    "creative",
    "knowledge",
    "agents",
    "operations",
    "security",
    "finance",
    "lifestyle",
    "other",
];

/// ClawHub 保留的话题名，命中即发布失败。判定作用于归一化后的形式。
pub const RESERVED_TOPICS: &[&str] = &[
    "approved",
    "audited",
    "certified",
    "clawhub",
    "community",
    "curated",
    "endorsed",
    "featured",
    "official",
    "officials",
    "openclaw",
    "recommended",
    "staff-pick",
    "trusted",
    "trusted-publisher",
    "verified",
];

/// 单个话题的最大长度。
pub const MAX_TOPIC_LEN: usize = 48;

/// 待上传的单个文件。`rel_path` 是相对技能根目录的路径，
/// 会作为 multipart part 的 filename 发送 —— ClawHub 用它还原目录结构。
#[derive(Debug, Clone)]
pub struct PublishFile {
    pub rel_path: String,
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// 发布预览里展示的文件条目（不含文件内容，避免跨 IPC 传大数据）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishFileEntry {
    pub rel_path: String,
    pub size: u64,
}

/// 发布前的预检结果：文件清单、体积、以及基于远端已发布版本推导的建议版本号。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPreview {
    pub files: Vec<PublishFileEntry>,
    pub total_bytes: u64,
    /// 默认 slug：有发布记录时沿用记录里的，否则从技能目录名推导。
    pub suggested_slug: String,
    /// 从 SKILL.md frontmatter 或目录名推导的默认展示名。
    pub suggested_display_name: String,
    /// 默认归属账号：沿用上次发布的 owner，否则为空（发布到当前用户名下）。
    pub suggested_owner_handle: Option<String>,
    /// 远端最新版本；技能尚未发布过时为 None。
    pub latest_version: Option<String>,
    /// 建议版本号：已知版本则 patch +1，否则 1.0.0。
    pub suggested_version: String,
    /// 上次成功发布的本地记录，供界面展示"已发布"状态。
    pub existing_record: Option<SkillPublishRecord>,
    /// 远端版本查询是否失败（网络/服务异常）。为 true 时建议版本号不可信，
    /// 界面需提示用户，避免静默按 1.0.0 重复发布。
    pub version_lookup_failed: bool,
    /// 远端已存在同名 slug 时给出的提示。
    pub warning: Option<String>,
}

/// 发布请求。由前端对话框收集后传入。
#[derive(Debug, Clone, Deserialize)]
pub struct PublishRequest {
    /// 本地技能的 instance_id，用于定位技能目录。
    pub instance_id: String,
    pub slug: String,
    pub display_name: String,
    pub version: String,
    #[serde(default)]
    pub changelog: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    /// 发布到组织时指定；留空则发布到当前认证用户名下。
    #[serde(default)]
    pub owner_handle: Option<String>,
    /// 用户是否已显式接受 MIT-0 许可条款。服务端强制校验，必须由用户勾选。
    pub accept_license_terms: bool,
}

/// 发布成功后的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    /// "published" | "pending" | 未知。pending 表示还在等安全扫描。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_status: Option<String>,
    /// 发布后的 ClawHub 页面地址，供前端"查看"跳转。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_url: Option<String>,
    pub version: String,
}

/// GET /api/v1/whoami 的结果，用于验证 token 并拿到默认 owner handle。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubIdentity {
    pub handle: Option<String>,
    pub display_name: Option<String>,
    pub image: Option<String>,
}
