use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::SkillScope;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillProviderKind {
    Filesystem,
    ConfigFile,
    Cli,
    Marketplace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SkillProviderCapabilities {
    pub list: bool,
    pub install: bool,
    pub enable: bool,
    pub disable: bool,
    pub update: bool,
    pub inspect: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillProvider {
    pub provider_id: String,
    pub kind: SkillProviderKind,
    pub display_name: String,
    pub root_path: Option<PathBuf>,
    pub detected: bool,
    pub cli_available: bool,
    pub reachable: Option<bool>,
    pub capabilities: SkillProviderCapabilities,
    pub skill_count: usize,
    pub enabled_count: usize,
    pub disabled_count: usize,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrcaTopic {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrcaInventory {
    pub cli_available: bool,
    pub available: bool,
    pub app_running: Option<bool>,
    pub runtime_reachable: Option<bool>,
    pub runtime_state: Option<String>,
    pub topics_available: bool,
    pub topics: Vec<OrcaTopic>,
    pub checked_at: u64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillProviderInventory {
    pub checked_at: u64,
    pub providers: Vec<SkillProvider>,
    pub orca: OrcaInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillBindingState {
    Enabled,
    Disabled,
    Missing,
    Conflict,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillBinding {
    pub artifact_id: String,
    pub skill_instance_id: String,
    pub provider_id: String,
    pub scope: SkillScope,
    pub state: SkillBindingState,
    pub source_path: Option<PathBuf>,
    pub target_path: Option<PathBuf>,
    pub last_checked_at: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillOperationAction {
    Enable,
    Disable,
    PresetApply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillBindingImpact {
    pub provider_id: String,
    pub display_name: String,
    pub root_path: Option<PathBuf>,
    pub shared: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationPreview {
    pub skill_instance_id: String,
    pub artifact_id: String,
    pub provider_id: String,
    pub scope: SkillScope,
    pub action: SkillOperationAction,
    pub impacts: Vec<SkillBindingImpact>,
    pub requires_confirmation: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationFailure {
    pub skill_instance_id: Option<String>,
    pub provider_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationReport {
    pub operation_id: String,
    pub action: SkillOperationAction,
    pub scope: Option<SkillScope>,
    pub project_id: Option<String>,
    pub provider_id: Option<String>,
    pub requested_count: usize,
    pub attempted_count: usize,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub failures: Vec<SkillOperationFailure>,
    pub impacts: Vec<SkillBindingImpact>,
    pub completed_at: u64,
}
