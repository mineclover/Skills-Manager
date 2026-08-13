use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StudioFeedbackTargetKind {
    Skill,
    SkillSetRelease,
    ActivationRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StudioFeedbackCode {
    Completed,
    Partial,
    Failed,
    WrongScope,
    InstructionGap,
    DependencyGap,
    SafetyConcern,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StudioEvidenceType {
    CommandResult,
    EvaluationAssertion,
    HumanConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StudioFeedbackEvent {
    pub id: String,
    pub target_kind: StudioFeedbackTargetKind,
    pub target_id: String,
    pub code: StudioFeedbackCode,
    pub evidence_type: StudioEvidenceType,
    /// Redacted, bounded summary only. Raw command output is deliberately never stored.
    pub evidence_summary: String,
    pub project_id: Option<String>,
    pub work_scope: Option<String>,
    pub provider_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationProviderOutcome {
    pub provider_id: String,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationRun {
    pub id: String,
    pub assignment_id: String,
    pub release_id: String,
    pub project_id: Option<String>,
    pub work_scope: String,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    #[serde(default)]
    pub provider_outcomes: Vec<ActivationProviderOutcome>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordStudioFeedbackRequest {
    pub target_kind: StudioFeedbackTargetKind,
    pub target_id: String,
    pub code: StudioFeedbackCode,
    pub evidence_type: StudioEvidenceType,
    pub evidence_summary: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub work_scope: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    Passed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationRecord {
    pub id: String,
    pub release_id: String,
    pub case_id: String,
    pub status: EvaluationStatus,
    pub evidence_type: StudioEvidenceType,
    pub evidence_summary: String,
    pub project_id: Option<String>,
    pub work_scope: Option<String>,
    pub provider_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseEvaluationSummary {
    pub release_id: String,
    pub total_count: u64,
    pub passed_count: u64,
    pub failed_count: u64,
    pub blocked_count: u64,
    pub last_evaluated_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordEvaluationRequest {
    pub release_id: String,
    pub case_id: String,
    pub status: EvaluationStatus,
    pub evidence_type: StudioEvidenceType,
    pub evidence_summary: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub work_scope: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StudioHealthStatus {
    Unknown,
    Healthy,
    NeedsReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseHealth {
    pub release_id: String,
    pub status: StudioHealthStatus,
    pub evaluated_count: u64,
    pub usage_count: u64,
    pub verified_success_rate: Option<f64>,
    pub correction_rate: Option<f64>,
    pub scope_mismatch_rate: Option<f64>,
    pub safety_incidents: u64,
    pub last_success_at: Option<i64>,
    pub freshness_days: Option<i64>,
}

/// Optional dimensions for reviewing a release in a concrete operating context.
/// Omitted dimensions intentionally aggregate across that dimension.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReleaseHealthContextRequest {
    pub release_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub work_scope: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewReason {
    InsufficientEvidence,
    ThresholdBreach,
    SafetyConcern,
    StaleEvaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewQueueItem {
    pub release_id: String,
    pub reason: ReviewReason,
    pub detail: String,
}

/// A human-actionable recommendation derived from repeated, evidence-backed outcomes.
/// Suggestions never mutate a contract, release, or provider binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseImprovementSuggestion {
    pub release_id: String,
    pub code: StudioFeedbackCode,
    pub occurrence_count: u64,
    pub title: String,
    pub rationale: String,
    pub suggested_action: String,
}
