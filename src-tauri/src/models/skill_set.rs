use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetMember {
    /// Canonical skill identity. Activation bindings remain provider- and scope-specific.
    pub skill_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetBlueprint {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub members: Vec<SkillSetMember>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetRelease {
    pub id: String,
    pub blueprint_id: String,
    pub blueprint_name: String,
    #[serde(default)]
    pub label: String,
    /// SHA-256 digest of the frozen release content.
    pub content_digest: String,
    pub members: Vec<SkillSetMember>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetAssignment {
    pub id: String,
    pub release_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Human-readable intended work scope, such as `upstream-integration`.
    pub work_scope: String,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SkillSetStore {
    pub schema_version: u32,
    #[serde(default)]
    pub blueprints: Vec<SkillSetBlueprint>,
    #[serde(default)]
    pub releases: Vec<SkillSetRelease>,
    #[serde(default)]
    pub assignments: Vec<SkillSetAssignment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSkillSetBlueprintRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSkillSetBlueprintRequest {
    pub blueprint_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSkillSetReleaseRequest {
    pub blueprint_id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignSkillSetReleaseRequest {
    pub release_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    pub work_scope: String,
    #[serde(default)]
    pub provider_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetSkillSetAssignmentActiveRequest {
    pub assignment_id: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPlanAction {
    Enable,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetActivationOperation {
    pub skill_id: String,
    pub skill_instance_id: String,
    pub tool_id: String,
    pub current_enabled: bool,
    pub action: ActivationPlanAction,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetActivationPlan {
    pub assignment_id: String,
    pub release_id: String,
    pub project_id: Option<String>,
    pub work_scope: String,
    pub operations: Vec<SkillSetActivationOperation>,
    pub missing_skill_ids: Vec<String>,
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSetActivationApplyResult {
    pub plan: SkillSetActivationPlan,
    pub activation_run_id: String,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub failures: Vec<String>,
}
