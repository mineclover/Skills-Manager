use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::config::{
    is_builtin_skill_activation_preset_id, PresetActivation, SkillActivationPreset,
};
use crate::models::{
    AppConfig, InstalledSkillPackage, SaveLocalSkillContractRequest, Skill, SkillBindingImpact,
    SkillContractSummary, SkillOperationAction, SkillOperationFailure, SkillOperationPreview,
    SkillOperationReport, SkillScope, DISABLED_TOOL_SKILL_SUFFIX,
};
use crate::services::{
    set_codex_plugin_enabled, ConfigManager, LinkStatus, LinkerService, ProviderInventoryService,
    ScannerService, SkillPackageService, WorkspaceService,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchSkillToolTargetKind {
    Skill,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchSkillToolAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSkillToolTarget {
    pub kind: BatchSkillToolTargetKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSetSkillToolsRequest {
    pub targets: Vec<BatchSkillToolTarget>,
    pub tool_ids: Vec<String>,
    pub action: BatchSkillToolAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSetSkillToolsFailure {
    pub target_kind: BatchSkillToolTargetKind,
    pub target_id: String,
    pub skill_id: Option<String>,
    pub tool_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSetSkillToolsResponse {
    pub requested_target_count: usize,
    pub requested_tool_count: usize,
    pub resolved_skill_count: usize,
    pub attempted_operation_count: usize,
    pub applied_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub failures: Vec<BatchSetSkillToolsFailure>,
    pub report: SkillOperationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedBatchSkillTarget {
    pub(crate) target_kind: BatchSkillToolTargetKind,
    pub(crate) target_id: String,
    pub(crate) skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchSkillToolOperation {
    pub(crate) target_kind: BatchSkillToolTargetKind,
    pub(crate) target_id: String,
    pub(crate) skill_id: String,
    pub(crate) tool_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchOperationPlan {
    pub(crate) operations: Vec<BatchSkillToolOperation>,
    pub(crate) skipped_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchFailureContext {
    pub(crate) target_kind: BatchSkillToolTargetKind,
    pub(crate) target_id: String,
    pub(crate) skill_id: Option<String>,
    pub(crate) tool_id: Option<String>,
    pub(crate) message: String,
}

impl BatchFailureContext {
    fn into_failure(self) -> BatchSetSkillToolsFailure {
        BatchSetSkillToolsFailure {
            target_kind: self.target_kind,
            target_id: self.target_id,
            skill_id: self.skill_id,
            tool_id: self.tool_id,
            message: self.message,
        }
    }
}

/// Shared skill-control use cases consumed by both the Tauri UI adapter and
/// command-line adapters. Interface-specific concerns such as cache invalidation
/// and output formatting stay outside this module.
pub struct SkillControlService;

impl SkillControlService {
    pub fn list_skills(project_id: Option<&str>) -> Result<Vec<Skill>, String> {
        let config = ConfigManager::new().load()?;
        ScannerService::scan_skills_for_scope(&config, project_id)
    }

    pub fn list_scoped_skills() -> Result<Vec<Skill>, String> {
        let config = ConfigManager::new().load()?;
        ScannerService::scan_scoped_skills(&config)
    }

    pub fn set_skill_enabled(
        instance_id: &str,
        tool_id: &str,
        enabled: bool,
    ) -> Result<SkillOperationReport, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_scoped_skills(&config)?;
        set_skill_enabled_from_skills(&config, &skills, None, instance_id, tool_id, enabled)
    }

    pub fn set_skill_enabled_for_scope(
        project_id: Option<&str>,
        instance_id: &str,
        tool_id: &str,
        enabled: bool,
    ) -> Result<SkillOperationReport, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        set_skill_enabled_from_skills(&config, &skills, project_id, instance_id, tool_id, enabled)
    }
}

fn set_skill_enabled_from_skills(
    config: &AppConfig,
    skills: &[Skill],
    project_id: Option<&str>,
    instance_id: &str,
    tool_id: &str,
    enabled: bool,
) -> Result<SkillOperationReport, String> {
    let skill = skills
        .iter()
        .find(|item| item.instance_id == instance_id)
        .ok_or_else(|| format!("Skill not found: {instance_id}"))?;

    let preview = ProviderInventoryService::preview_binding_operation_with_skills(
        &config,
        &skills,
        project_id,
        instance_id,
        tool_id,
        enabled,
    )?;
    let mut report = operation_report_from_preview(&preview, project_id);
    if skill.is_enabled_for(tool_id) == enabled {
        report.skipped_count = 1;
        return Ok(report);
    }

    report.attempted_count = 1;
    match apply_skill_tool_enabled_from_skills(
        &config,
        &skills,
        instance_id,
        tool_id,
        enabled,
        Some(skill.path.as_path()),
    ) {
        Ok(()) => report.applied_count = 1,
        Err(message) => {
            report.failed_count = 1;
            report.failures.push(SkillOperationFailure {
                skill_instance_id: Some(instance_id.to_string()),
                provider_id: Some(tool_id.to_string()),
                message,
            });
        }
    }
    Ok(report)
}

impl SkillControlService {
    pub fn delete_skill(instance_id: &str) -> Result<(), String> {
        let config = ConfigManager::new().load()?;
        delete_skill_from_disk(&config, instance_id)
    }

    pub fn create_skill(name: &str, description: Option<&str>) -> Result<Skill, String> {
        let manager = ConfigManager::new();
        let config = manager.load()?;
        let id: String = name
            .trim()
            .to_lowercase()
            .chars()
            .map(|character| if character == ' ' { '-' } else { character })
            .filter(|character| {
                character.is_alphanumeric() || *character == '-' || *character == '_'
            })
            .collect();

        if id.is_empty() {
            return Err("Invalid skill name".to_string());
        }

        let skill_path = config.skills_dir.join(&id);
        if skill_path.exists() {
            return Err(format!("Skill \"{id}\" already exists"));
        }

        std::fs::create_dir_all(&skill_path)
            .map_err(|error| format!("Failed to create skill folder: {error}"))?;
        let description = description
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Replace with description of the skill and when Claude should use it.");
        let content = format!(
            "---\nname: {id}\ndescription: {description}\n---\n\n# Insert instructions below\n"
        );
        let skill_md_path = skill_path.join("SKILL.md");
        std::fs::write(&skill_md_path, content)
            .map_err(|error| format!("Failed to write SKILL.md: {error}"))?;

        ScannerService::load_skill_with_config(&skill_path, &config)
    }

    pub fn save_local_skill_contract(
        request: SaveLocalSkillContractRequest,
    ) -> Result<SkillContractSummary, String> {
        let instance_id = request.skill_instance_id.trim();
        if instance_id.is_empty() {
            return Err("Skill instance id is required for local contract metadata".to_string());
        }
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let project_id = instance_id
            .strip_prefix("project:")
            .and_then(|value| value.split_once(':').map(|(project_id, _)| project_id));
        let skill = ScannerService::scan_skills_for_scope(&config, project_id)?
            .into_iter()
            .find(|skill| skill.instance_id == instance_id)
            .ok_or_else(|| format!("Skill instance not found: {instance_id}"))?;
        if skill
            .path
            .join(crate::models::SKILL_CONTRACT_FILE_NAME)
            .exists()
        {
            return Err(
                "A portable skill-manager.yaml already exists and takes precedence".to_string(),
            );
        }
        config
            .skill_metadata
            .entry(instance_id.to_string())
            .or_default()
            .local_contract = Some(request.contract.clone());
        manager.save(&config)?;
        Ok(SkillContractSummary::from_local_metadata(request.contract))
    }

    pub fn import_skills_to_hub(skill_paths: &[String]) -> Result<(), String> {
        for path in skill_paths {
            LinkerService::import_to_hub(path)?;
        }
        Ok(())
    }

    pub fn apply_preset(preset_id: &str) -> Result<SkillOperationReport, String> {
        let manager = ConfigManager::new();
        let config = manager.load()?;
        let project_id = config.active_project_id.clone();
        let skills = ScannerService::scan_scoped_skills(&config)?;
        apply_preset_with_skills(preset_id, config, skills, project_id.as_deref())
    }

    pub fn apply_preset_for_scope(
        preset_id: &str,
        project_id: Option<&str>,
    ) -> Result<SkillOperationReport, String> {
        let manager = ConfigManager::new();
        let config = manager.load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        apply_preset_with_skills(preset_id, config, skills, project_id)
    }

    pub fn apply_preset_for_target(
        preset_id: &str,
        project_id: Option<&str>,
        tool_id: &str,
    ) -> Result<SkillOperationReport, String> {
        let manager = ConfigManager::new();
        let config = manager.load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        apply_preset_to_target_with_skills(preset_id, tool_id, config, skills, project_id)
    }

    pub fn clear_active_preset() -> Result<(), String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        config.active_preset_id = None;
        manager.save(&config)
    }

    pub fn create_preset(
        name: &str,
        description: Option<&str>,
        copy_current_state: bool,
        project_id: Option<&str>,
        tool_id: Option<&str>,
    ) -> Result<SkillActivationPreset, String> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err("Preset name is required".to_string());
        }

        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let activations = if copy_current_state {
            let tool_id = tool_id.ok_or_else(|| "Target tool is required".to_string())?;
            let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
            vec![PresetActivation {
                tool_id: tool_id.to_string(),
                skill_ids: skill_ids_for_target(&skills, tool_id, true),
            }]
        } else {
            Vec::new()
        };
        let preset = SkillActivationPreset {
            id: format!("preset-{}", current_timestamp_millis()),
            name: trimmed_name.to_string(),
            description: description
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            activations,
        };
        config.presets.push(preset.clone());
        manager.save(&config)?;
        Ok(preset)
    }

    pub fn delete_preset(preset_id: &str) -> Result<(), String> {
        if is_builtin_skill_activation_preset_id(preset_id) {
            return Err("Built-in presets cannot be deleted".to_string());
        }
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let original_len = config.presets.len();
        config.presets.retain(|preset| preset.id != preset_id);
        if config.presets.len() == original_len {
            return Err(format!("Preset not found: {preset_id}"));
        }
        if config.active_preset_id.as_deref() == Some(preset_id) {
            config.active_preset_id = None;
        }
        manager.save(&config)
    }

    pub fn capture_preset(
        preset_id: &str,
        project_id: Option<&str>,
        tool_id: &str,
    ) -> Result<SkillActivationPreset, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        let skill_ids = skill_ids_for_target(&skills, tool_id, true);
        let preset = config
            .presets
            .iter_mut()
            .find(|preset| preset.id == preset_id)
            .ok_or_else(|| format!("Preset not found: {preset_id}"))?;
        preset
            .activations
            .retain(|activation| activation.tool_id != tool_id);
        preset.activations.push(PresetActivation {
            tool_id: tool_id.to_string(),
            skill_ids,
        });
        let updated = preset.clone();
        manager.save(&config)?;
        Ok(updated)
    }

    pub fn set_preset_skill(
        preset_id: &str,
        project_id: Option<&str>,
        tool_id: &str,
        skill_id: &str,
        enabled: bool,
    ) -> Result<SkillActivationPreset, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        if !skills.iter().any(|skill| {
            skill.instance_id == skill_id
                && (skill.scope != SkillScope::Tool || skill.tool_id.as_deref() == Some(tool_id))
        }) {
            return Err(format!(
                "Skill is not available for tool {tool_id}: {skill_id}"
            ));
        }

        let preset = config
            .presets
            .iter_mut()
            .find(|preset| preset.id == preset_id)
            .ok_or_else(|| format!("Preset not found: {preset_id}"))?;
        if enabled {
            if let Some(activation) = preset
                .activations
                .iter_mut()
                .find(|activation| activation.tool_id == tool_id)
            {
                if !activation.skill_ids.iter().any(|id| id == skill_id) {
                    activation.skill_ids.push(skill_id.to_string());
                }
            } else {
                preset.activations.push(PresetActivation {
                    tool_id: tool_id.to_string(),
                    skill_ids: vec![skill_id.to_string()],
                });
            }
        } else if let Some(activation) = preset
            .activations
            .iter_mut()
            .find(|activation| activation.tool_id == tool_id)
        {
            activation.skill_ids.retain(|id| id != skill_id);
        }
        let updated = preset.clone();
        manager.save(&config)?;
        Ok(updated)
    }

    pub fn set_preset_all(
        preset_id: &str,
        project_id: Option<&str>,
        tool_id: &str,
        enabled: bool,
    ) -> Result<SkillActivationPreset, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        let skill_ids = if enabled {
            skill_ids_for_target(&skills, tool_id, false)
        } else {
            Vec::new()
        };
        let preset = config
            .presets
            .iter_mut()
            .find(|preset| preset.id == preset_id)
            .ok_or_else(|| format!("Preset not found: {preset_id}"))?;
        if let Some(activation) = preset
            .activations
            .iter_mut()
            .find(|activation| activation.tool_id == tool_id)
        {
            activation.skill_ids = skill_ids;
        } else {
            preset.activations.push(PresetActivation {
                tool_id: tool_id.to_string(),
                skill_ids,
            });
        }
        let updated = preset.clone();
        manager.save(&config)?;
        Ok(updated)
    }

    pub fn batch_set_skill_tools(
        request: BatchSetSkillToolsRequest,
    ) -> Result<BatchSetSkillToolsResponse, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_scoped_skills(&config)?;
        let all_skills = skills.clone();
        let skill_packages = SkillPackageService::list_discovered_packages(&config.skills_dir)?;
        let skills_by_instance_id: HashMap<String, Skill> = skills
            .into_iter()
            .map(|skill| (skill.instance_id.clone(), skill))
            .collect();
        let packages_by_id: HashMap<String, InstalledSkillPackage> = skill_packages
            .into_iter()
            .map(|skill_package| (skill_package.package_id.clone(), skill_package))
            .collect();

        let requested_target_count = request.targets.len();
        let requested_tool_count = request.tool_ids.len();
        let (resolved_targets, mut failures) =
            resolve_batch_targets(&request.targets, &skills_by_instance_id, &packages_by_id);
        let resolved_skill_ids: HashSet<String> = resolved_targets
            .iter()
            .map(|target| target.skill_id.clone())
            .collect();
        let (operation_plan, operation_failures) = build_batch_operations(
            &resolved_targets,
            &request.tool_ids,
            &skills_by_instance_id,
            &config,
            &request.action,
        );
        failures.extend(operation_failures);

        let mut applied_count = 0;
        let mut impacts = Vec::new();
        let should_enable = matches!(request.action, BatchSkillToolAction::Enable);
        for operation in &operation_plan.operations {
            if let Ok(preview) = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                &all_skills,
                None,
                &operation.skill_id,
                &operation.tool_id,
                should_enable,
            ) {
                extend_unique_impacts(&mut impacts, preview.impacts);
            }
            let skill_path = skills_by_instance_id
                .get(&operation.skill_id)
                .map(|skill| skill.path.as_path());
            if let Err(message) = apply_skill_tool_enabled(
                &config,
                &operation.skill_id,
                &operation.tool_id,
                should_enable,
                skill_path,
            ) {
                failures.push(batch_failure(
                    operation.target_kind.clone(),
                    operation.target_id.clone(),
                    Some(operation.skill_id.clone()),
                    Some(operation.tool_id.clone()),
                    message,
                ));
                continue;
            }
            applied_count += 1;
        }

        let failures: Vec<BatchSetSkillToolsFailure> = failures
            .into_iter()
            .map(BatchFailureContext::into_failure)
            .collect();
        let failed_count = failures.len();
        let report_failures = failures
            .iter()
            .map(|failure| SkillOperationFailure {
                skill_instance_id: failure.skill_id.clone(),
                provider_id: failure.tool_id.clone(),
                message: failure.message.clone(),
            })
            .collect::<Vec<_>>();
        let report = SkillOperationReport {
            operation_id: new_operation_id(),
            action: if should_enable {
                SkillOperationAction::Enable
            } else {
                SkillOperationAction::Disable
            },
            scope: None,
            project_id: None,
            provider_id: None,
            requested_count: requested_target_count.saturating_mul(requested_tool_count),
            attempted_count: operation_plan.operations.len(),
            applied_count,
            skipped_count: operation_plan.skipped_count,
            failed_count,
            failures: report_failures,
            impacts,
            completed_at: current_timestamp(),
        };

        Ok(BatchSetSkillToolsResponse {
            requested_target_count,
            requested_tool_count,
            resolved_skill_count: resolved_skill_ids.len(),
            attempted_operation_count: operation_plan.operations.len(),
            applied_count,
            skipped_count: operation_plan.skipped_count,
            failed_count,
            failures,
            report,
        })
    }
}

fn new_operation_id() -> String {
    format!("skill-operation-{}", current_timestamp_millis())
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn extend_unique_impacts(target: &mut Vec<SkillBindingImpact>, impacts: Vec<SkillBindingImpact>) {
    for impact in impacts {
        if !target.iter().any(|existing| {
            existing.provider_id == impact.provider_id && existing.root_path == impact.root_path
        }) {
            target.push(impact);
        }
    }
}

fn operation_report_from_preview(
    preview: &SkillOperationPreview,
    project_id: Option<&str>,
) -> SkillOperationReport {
    SkillOperationReport {
        operation_id: new_operation_id(),
        action: preview.action.clone(),
        scope: Some(preview.scope.clone()),
        project_id: project_id.map(ToString::to_string),
        provider_id: Some(preview.provider_id.clone()),
        requested_count: 1,
        attempted_count: 0,
        applied_count: 0,
        skipped_count: 0,
        failed_count: 0,
        failures: Vec::new(),
        impacts: preview.impacts.clone(),
        completed_at: current_timestamp(),
    }
}

fn current_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn skill_ids_for_target(skills: &[Skill], tool_id: &str, only_enabled: bool) -> Vec<String> {
    skills
        .iter()
        .filter(|skill| {
            (skill.scope != SkillScope::Tool || skill.tool_id.as_deref() == Some(tool_id))
                && (!only_enabled || skill.enabled.get(tool_id).copied().unwrap_or(false))
        })
        .map(|skill| skill.instance_id.clone())
        .collect()
}

pub(crate) fn batch_failure(
    target_kind: BatchSkillToolTargetKind,
    target_id: impl Into<String>,
    skill_id: Option<String>,
    tool_id: Option<String>,
    message: impl Into<String>,
) -> BatchFailureContext {
    BatchFailureContext {
        target_kind,
        target_id: target_id.into(),
        skill_id,
        tool_id,
        message: message.into(),
    }
}

pub(crate) fn resolve_batch_targets(
    targets: &[BatchSkillToolTarget],
    skills_by_instance_id: &HashMap<String, Skill>,
    packages_by_id: &HashMap<String, InstalledSkillPackage>,
) -> (Vec<ResolvedBatchSkillTarget>, Vec<BatchFailureContext>) {
    let mut resolved = Vec::new();
    let mut failures = Vec::new();

    for target in targets {
        match target.kind {
            BatchSkillToolTargetKind::Skill => {
                if skills_by_instance_id.contains_key(&target.id) {
                    resolved.push(ResolvedBatchSkillTarget {
                        target_kind: BatchSkillToolTargetKind::Skill,
                        target_id: target.id.clone(),
                        skill_id: target.id.clone(),
                    });
                } else {
                    failures.push(batch_failure(
                        BatchSkillToolTargetKind::Skill,
                        target.id.clone(),
                        Some(target.id.clone()),
                        None,
                        format!("Skill not found: {}", target.id),
                    ));
                }
            }
            BatchSkillToolTargetKind::Group => {
                let Some(skill_package) = packages_by_id.get(&target.id) else {
                    failures.push(batch_failure(
                        BatchSkillToolTargetKind::Group,
                        target.id.clone(),
                        None,
                        None,
                        format!("Skill group not found: {}", target.id),
                    ));
                    continue;
                };

                for skill_id in &skill_package.installed_members {
                    let matching_skills = skills_by_instance_id
                        .values()
                        .filter(|skill| &skill.id == skill_id)
                        .collect::<Vec<_>>();

                    if matching_skills.is_empty() {
                        failures.push(batch_failure(
                            BatchSkillToolTargetKind::Group,
                            target.id.clone(),
                            Some(skill_id.clone()),
                            None,
                            format!("Skill not found: {}", skill_id),
                        ));
                        continue;
                    }

                    let Some(preferred_skill) = matching_skills
                        .iter()
                        .find(|skill| skill.scope == SkillScope::Global)
                    else {
                        failures.push(batch_failure(
                            BatchSkillToolTargetKind::Group,
                            target.id.clone(),
                            Some(skill_id.clone()),
                            None,
                            format!("Global skill not found for group member: {}", skill_id),
                        ));
                        continue;
                    };

                    resolved.push(ResolvedBatchSkillTarget {
                        target_kind: BatchSkillToolTargetKind::Group,
                        target_id: target.id.clone(),
                        skill_id: preferred_skill.instance_id.clone(),
                    });
                }
            }
        }
    }

    (resolved, failures)
}

pub(crate) fn build_batch_operations(
    resolved_targets: &[ResolvedBatchSkillTarget],
    tool_ids: &[String],
    skills_by_instance_id: &HashMap<String, Skill>,
    config: &AppConfig,
    action: &BatchSkillToolAction,
) -> (BatchOperationPlan, Vec<BatchFailureContext>) {
    let mut failures = Vec::new();
    let mut seen = HashSet::new();
    let mut operations = Vec::new();
    let mut skipped_count = 0;
    let should_enable = matches!(action, BatchSkillToolAction::Enable);

    for resolved_target in resolved_targets {
        let Some(skill) = skills_by_instance_id.get(&resolved_target.skill_id) else {
            failures.push(batch_failure(
                resolved_target.target_kind.clone(),
                resolved_target.target_id.clone(),
                Some(resolved_target.skill_id.clone()),
                None,
                format!("Skill not found: {}", resolved_target.skill_id),
            ));
            continue;
        };

        for tool_id in tool_ids {
            if !seen.insert((resolved_target.skill_id.clone(), tool_id.clone())) {
                continue;
            }

            if skill.scope == SkillScope::Tool && skill.tool_id.as_deref() != Some(tool_id) {
                skipped_count += 1;
                continue;
            }

            if config.get_tool_config(tool_id).is_none() {
                failures.push(batch_failure(
                    resolved_target.target_kind.clone(),
                    resolved_target.target_id.clone(),
                    Some(resolved_target.skill_id.clone()),
                    Some(tool_id.clone()),
                    format!("Tool not found: {}", tool_id),
                ));
                continue;
            }

            if skill.is_enabled_for(tool_id) == should_enable {
                skipped_count += 1;
                continue;
            }

            operations.push(BatchSkillToolOperation {
                target_kind: resolved_target.target_kind.clone(),
                target_id: resolved_target.target_id.clone(),
                skill_id: resolved_target.skill_id.clone(),
                tool_id: tool_id.clone(),
            });
        }
    }

    (
        BatchOperationPlan {
            operations,
            skipped_count,
        },
        failures,
    )
}

pub fn resolve_skill_source_path(_config: &AppConfig, skill: &Skill) -> std::path::PathBuf {
    skill.path.clone()
}

pub fn load_skill_by_instance_id(config: &AppConfig, instance_id: &str) -> Result<Skill, String> {
    ScannerService::scan_scoped_skills(config)?
        .into_iter()
        .find(|item| item.instance_id == instance_id)
        .ok_or_else(|| format!("Skill not found: {instance_id}"))
}

pub fn apply_skill_tool_enabled(
    config: &AppConfig,
    instance_id: &str,
    tool_id: &str,
    enabled: bool,
    skill_path: Option<&Path>,
) -> Result<(), String> {
    let skills = ScannerService::scan_scoped_skills(config)?;
    apply_skill_tool_enabled_from_skills(config, &skills, instance_id, tool_id, enabled, skill_path)
}

pub fn apply_skill_tool_enabled_from_skills(
    config: &AppConfig,
    skills: &[Skill],
    instance_id: &str,
    tool_id: &str,
    enabled: bool,
    skill_path: Option<&Path>,
) -> Result<(), String> {
    let tool_config = config
        .get_tool_config(tool_id)
        .ok_or_else(|| format!("Tool not found: {tool_id}"))?;
    ProviderInventoryService::ensure_activation_capability(config, tool_id, enabled)?;

    let skill = skills
        .iter()
        .find(|item| item.instance_id == instance_id)
        .ok_or_else(|| format!("Skill not found: {instance_id}"))?;

    // A Tool-scoped skill belongs to the tool directory where it was found.
    // Sharing it with another tool requires importing it into the hub first.
    if skill.scope == SkillScope::Tool {
        if skill.tool_id.as_deref() != Some(tool_id) {
            return Err(format!(
                "Tool-scoped skill {instance_id} belongs to another tool"
            ));
        }

        if tool_id == "codex" {
            set_codex_plugin_enabled(&tool_config.config_path, &skill.id, enabled)?;
        } else {
            rename_tool_skill_for_state(&skill.path, &tool_config.skills_path, &skill.id, enabled)?;
        }

        return Ok(());
    }

    let (target_skills_dir, target_config_dir) = activation_target_paths(config, skill, tool_id);
    let skill_is_directly_registered = target_skills_dir.join(&skill.id) == skill.path;
    let skill_is_directly_disabled = skill
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name == format!("{}{}", skill.id, DISABLED_TOOL_SKILL_SUFFIX)
                && skill.path.parent() == Some(target_skills_dir.as_path())
        });

    if enabled {
        if skill_is_directly_disabled {
            return rename_tool_skill_for_state(&skill.path, &target_skills_dir, &skill.id, true);
        }
        if skill_is_directly_registered {
            return Ok(());
        }

        let skill_path = match skill_path {
            Some(path) => path.to_path_buf(),
            None => resolve_skill_source_path(config, skill),
        };
        if !skill_path.exists() {
            return Err(format!("Skill not found: {instance_id}"));
        }

        LinkerService::enable_skill_for_tool(&skill_path, &target_skills_dir, &skill.id, tool_id)?;

        if tool_id == "codex" {
            if let Err(error) = set_codex_plugin_enabled(&target_config_dir, &skill.id, true) {
                let _ =
                    LinkerService::disable_skill_for_tool(&target_skills_dir, &skill.id, tool_id);
                return Err(format!("Failed to update codex config.toml: {error}"));
            }
        }

        return Ok(());
    }

    let current_link_status = LinkerService::check_link_for_scoped_skill(
        &skill.path,
        &target_skills_dir,
        &skill.id,
        tool_id,
        &skill.scope,
    );
    if skill_is_directly_registered {
        return rename_tool_skill_for_state(&skill.path, &target_skills_dir, &skill.id, false);
    }
    let should_restore_link = current_link_status == LinkStatus::Valid;
    let disable_result = match current_link_status {
        LinkStatus::Valid => {
            LinkerService::disable_skill_for_tool(&target_skills_dir, &skill.id, tool_id)
        }
        LinkStatus::Missing => Ok(()),
        _ => Err(format!(
            "Skill target belongs to another instance: {instance_id}"
        )),
    };

    disable_result?;

    if tool_id == "codex" {
        if let Err(error) = set_codex_plugin_enabled(&target_config_dir, &skill.id, false) {
            if should_restore_link {
                let skill_path = resolve_skill_source_path(config, skill);
                let _ = LinkerService::enable_skill_for_tool(
                    &skill_path,
                    &target_skills_dir,
                    &skill.id,
                    tool_id,
                );
            }
            return Err(format!("Failed to update codex config.toml: {error}"));
        }
    }

    Ok(())
}

fn activation_target_paths(
    config: &AppConfig,
    skill: &Skill,
    tool_id: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    if skill.scope == SkillScope::Project {
        if let Some(project) = skill.project_id.as_deref().and_then(|project_id| {
            config
                .projects
                .iter()
                .find(|project| project.id == project_id)
        }) {
            if let Some(skills_dir) = WorkspaceService::project_tool_skills_dir(project, tool_id) {
                let config_dir = WorkspaceService::project_tool_config_dir(project, tool_id)
                    .unwrap_or_else(|| skills_dir.join(".."));
                return (skills_dir, config_dir);
            }
        }
    }

    let tool_config = config
        .get_tool_config(tool_id)
        .expect("activation target is resolved after provider validation");
    (tool_config.skills_path, tool_config.config_path)
}

pub fn rename_tool_skill_for_state(
    current_path: &Path,
    skills_dir: &Path,
    skill_id: &str,
    enabled: bool,
) -> Result<(), String> {
    if current_path.strip_prefix(skills_dir).is_err() {
        return Err(format!(
            "Tool skill path is outside the configured skills directory: {}",
            current_path.display()
        ));
    }

    let parent = current_path
        .parent()
        .ok_or_else(|| "Invalid tool skill path parent".to_string())?;
    let target_name = if enabled {
        skill_id.to_string()
    } else {
        format!("{skill_id}{DISABLED_TOOL_SKILL_SUFFIX}")
    };
    let target_path = parent.join(target_name);

    if target_path == current_path {
        return Ok(());
    }

    if target_path.exists() || target_path.symlink_metadata().is_ok() {
        return Err(format!(
            "Cannot change tool skill state because the target already exists: {}",
            target_path.display()
        ));
    }

    std::fs::rename(current_path, &target_path)
        .map_err(|error| format!("Failed to rename tool skill directory: {error}"))
}

pub fn delete_skill_from_disk(config: &AppConfig, instance_id: &str) -> Result<(), String> {
    let skill = load_skill_by_instance_id(config, instance_id)?;
    let skill_path = resolve_skill_source_path(config, &skill);
    if !skill_path.exists() {
        return Err(format!("Skill not found: {instance_id}"));
    }

    for (tool_id, tool_config) in config.collect_tool_configs() {
        match LinkerService::check_link_for_scoped_skill(
            &skill.path,
            &tool_config.skills_path,
            &skill.id,
            &tool_id,
            &skill.scope,
        ) {
            LinkStatus::Valid => {
                let _ = LinkerService::disable_skill_for_tool(
                    &tool_config.skills_path,
                    &skill.id,
                    &tool_id,
                );
            }
            LinkStatus::Missing => {}
            _ => {}
        }
    }

    std::fs::remove_dir_all(&skill_path)
        .map_err(|error| format!("Failed to delete skill folder: {error}"))
}

fn apply_preset_with_skills(
    preset_id: &str,
    mut config: AppConfig,
    skills: Vec<Skill>,
    project_id: Option<&str>,
) -> Result<SkillOperationReport, String> {
    let manager = ConfigManager::new();
    let preset = config
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Preset not found: {preset_id}"))?
        .clone();

    let active_mappings = preset
        .activations
        .iter()
        .map(|activation| {
            (
                activation.tool_id.clone(),
                activation.skill_ids.iter().cloned().collect::<HashSet<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut report = SkillOperationReport {
        operation_id: new_operation_id(),
        action: SkillOperationAction::PresetApply,
        scope: project_id.map(|_| SkillScope::Project),
        project_id: project_id.map(ToString::to_string),
        provider_id: None,
        requested_count: 0,
        attempted_count: 0,
        applied_count: 0,
        skipped_count: 0,
        failed_count: 0,
        failures: Vec::new(),
        impacts: Vec::new(),
        completed_at: current_timestamp(),
    };

    for (tool_id, tool_config) in config.collect_tool_configs() {
        let tool_has_installation = tool_config.detected
            || tool_config.config_path.exists()
            || tool_config.skills_path.exists();
        if !tool_has_installation {
            continue;
        }

        let Some(active_set) = active_mappings.get(&tool_id) else {
            // A preset is agent-specific. An absent mapping means that this
            // preset does not target the tool, not that every skill should be
            // disabled for it.
            continue;
        };
        for skill in &skills {
            if skill.scope == SkillScope::Tool && skill.tool_id.as_deref() != Some(&tool_id) {
                continue;
            }

            let should_be_enabled =
                active_set.contains(&skill.instance_id) || active_set.contains(&skill.id);
            report.requested_count += 1;
            if skill.is_enabled_for(&tool_id) == should_be_enabled {
                report.skipped_count += 1;
                continue;
            }
            report.attempted_count += 1;
            if let Ok(preview) = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                &skills,
                project_id,
                &skill.instance_id,
                &tool_id,
                should_be_enabled,
            ) {
                extend_unique_impacts(&mut report.impacts, preview.impacts);
            }
            match apply_skill_tool_enabled_from_skills(
                &config,
                &skills,
                &skill.instance_id,
                &tool_id,
                should_be_enabled,
                Some(skill.path.as_path()),
            ) {
                Ok(()) => report.applied_count += 1,
                Err(message) => {
                    report.failed_count += 1;
                    report.failures.push(SkillOperationFailure {
                        skill_instance_id: Some(skill.instance_id.clone()),
                        provider_id: Some(tool_id.clone()),
                        message,
                    });
                }
            }
        }
    }

    config.active_preset_id = Some(preset_id.to_string());
    manager.save(&config)?;
    report.completed_at = current_timestamp();
    Ok(report)
}

pub fn apply_preset_to_target_with_skills(
    preset_id: &str,
    target_tool_id: &str,
    mut config: AppConfig,
    skills: Vec<Skill>,
    project_id: Option<&str>,
) -> Result<SkillOperationReport, String> {
    let manager = ConfigManager::new();
    let tool_config = config
        .get_tool_config(target_tool_id)
        .ok_or_else(|| format!("Tool not found: {target_tool_id}"))?;
    let tool_has_installation = tool_config.detected
        || tool_config.config_path.exists()
        || tool_config.skills_path.exists();
    if !tool_has_installation {
        return Err(format!("Tool is not available: {target_tool_id}"));
    }

    let preset = config
        .presets
        .iter()
        .find(|preset| preset.id == preset_id)
        .ok_or_else(|| format!("Preset not found: {preset_id}"))?
        .clone();
    let active_set = preset
        .activations
        .iter()
        .find(|activation| activation.tool_id == target_tool_id)
        .map(|activation| activation.skill_ids.iter().cloned().collect::<HashSet<_>>())
        .ok_or_else(|| {
            format!(
                "Preset is not configured for tool: {target_tool_id}. Configure this agent in the preset before applying it."
            )
        })?;
    let mut report = SkillOperationReport {
        operation_id: new_operation_id(),
        action: SkillOperationAction::PresetApply,
        scope: project_id.map(|_| SkillScope::Project),
        project_id: project_id.map(ToString::to_string),
        provider_id: Some(target_tool_id.to_string()),
        requested_count: 0,
        attempted_count: 0,
        applied_count: 0,
        skipped_count: 0,
        failed_count: 0,
        failures: Vec::new(),
        impacts: Vec::new(),
        completed_at: current_timestamp(),
    };

    for skill in skills.iter().filter(|skill| {
        skill.scope != SkillScope::Tool || skill.tool_id.as_deref() == Some(target_tool_id)
    }) {
        let should_be_enabled =
            active_set.contains(&skill.instance_id) || active_set.contains(&skill.id);
        report.requested_count += 1;
        if skill.is_enabled_for(target_tool_id) == should_be_enabled {
            report.skipped_count += 1;
            continue;
        }
        report.attempted_count += 1;
        if let Ok(preview) = ProviderInventoryService::preview_binding_operation_with_skills(
            &config,
            &skills,
            project_id,
            &skill.instance_id,
            target_tool_id,
            should_be_enabled,
        ) {
            extend_unique_impacts(&mut report.impacts, preview.impacts);
        }
        match apply_skill_tool_enabled_from_skills(
            &config,
            &skills,
            &skill.instance_id,
            target_tool_id,
            should_be_enabled,
            Some(skill.path.as_path()),
        ) {
            Ok(()) => report.applied_count += 1,
            Err(message) => {
                report.failed_count += 1;
                report.failures.push(SkillOperationFailure {
                    skill_instance_id: Some(skill.instance_id.clone()),
                    provider_id: Some(target_tool_id.to_string()),
                    message,
                });
            }
        }
    }

    config.active_preset_id = Some(preset_id.to_string());
    manager.save(&config)?;
    report.completed_at = current_timestamp();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_preset_to_target_with_skills, apply_skill_tool_enabled_from_skills,
        SkillControlService,
    };
    use crate::models::{
        AppConfig, PresetActivation, ProjectBinding, SaveLocalSkillContractRequest, Skill,
        SkillActivationPreset, SkillContract, SkillContractSource, SkillScope, ToolConfig,
    };
    use crate::services::{ConfigManager, ScannerService};
    use crate::test_support::with_temp_home;
    use std::fs;

    #[test]
    fn local_contract_is_saved_for_the_exact_skill_instance_and_rejects_sidecars() {
        with_temp_home(|_| {
            let mut config = AppConfig::default();
            config.initialized = true;
            let skill_dir = config.skills_dir.join("local-contract");
            fs::create_dir_all(&skill_dir).expect("create skill");
            fs::write(
                skill_dir.join("SKILL.md"),
                "---\nname: local-contract\n---\n",
            )
            .expect("write skill");
            ConfigManager::new().save(&config).expect("save config");

            let summary =
                SkillControlService::save_local_skill_contract(SaveLocalSkillContractRequest {
                    skill_instance_id: "global:local-contract".to_string(),
                    contract: SkillContract::default(),
                })
                .expect("save local contract");
            assert_eq!(summary.source, Some(SkillContractSource::LocalMetadata));

            let loaded = ConfigManager::new().load().expect("load config");
            assert!(loaded
                .skill_metadata
                .get("global:local-contract")
                .and_then(|metadata| metadata.local_contract.as_ref())
                .is_some());

            fs::write(skill_dir.join("skill-manager.yaml"), "schema_version: 1\n")
                .expect("write sidecar");
            let error =
                SkillControlService::save_local_skill_contract(SaveLocalSkillContractRequest {
                    skill_instance_id: "global:local-contract".to_string(),
                    contract: SkillContract::default(),
                })
                .expect_err("portable sidecar must take precedence");
            assert!(error.contains("takes precedence"));
        });
    }

    #[test]
    fn target_preset_rejects_an_agent_without_explicit_configuration() {
        with_temp_home(|home| {
            let mut config = AppConfig::default();
            config.tools.insert(
                "claude-code".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: home.join(".claude").join("skills"),
                    config_path: home.join(".claude"),
                },
            );
            config.presets = vec![SkillActivationPreset {
                id: "codex-only".to_string(),
                name: "Codex only".to_string(),
                description: None,
                activations: vec![PresetActivation {
                    tool_id: "codex".to_string(),
                    skill_ids: vec!["tool:codex:code-review".to_string()],
                }],
            }];

            let error = apply_preset_to_target_with_skills(
                "codex-only",
                "claude-code",
                config,
                Vec::new(),
                None,
            )
            .expect_err("unconfigured agent should be rejected");
            assert!(error.contains("not configured for tool: claude-code"));
        });
    }

    #[test]
    fn matt_planning_preset_selectively_activates_codex_direct_skills() {
        with_temp_home(|home| {
            let codex_skills = home.join(".codex").join("skills");
            for name in ["ask-matt", "code-review"] {
                let skill_dir = codex_skills.join(name);
                fs::create_dir_all(&skill_dir).expect("create Codex skill");
                fs::write(
                    skill_dir.join("SKILL.md"),
                    format!("---\nname: {name}\n---\n"),
                )
                .expect("write Codex skill");
            }

            let mut config = AppConfig::default();
            config.initialized = true;
            config.tools.insert(
                "codex".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: codex_skills.clone(),
                    config_path: home.join(".codex"),
                },
            );
            let preset_id = "builtin-matt-planning";
            let skills = ScannerService::scan_skills_for_scope(&config, None)
                .expect("scan Codex direct skills");

            let report =
                apply_preset_to_target_with_skills(preset_id, "codex", config, skills, None)
                    .expect("apply planning preset");

            assert_eq!(report.failed_count, 0);
            assert!(codex_skills.join("ask-matt").exists());
            assert!(codex_skills.join("code-review").exists());
            let codex_config = fs::read_to_string(home.join(".codex").join("config.toml"))
                .expect("read Codex plugin state");
            assert!(codex_config.contains("[plugins.\"code-review\"]"));
            assert!(codex_config.contains("enabled = false"));
            let loaded = crate::services::ConfigManager::new()
                .load()
                .expect("load applied config");
            assert_eq!(loaded.active_preset_id.as_deref(), Some(preset_id));
            let refreshed = ScannerService::scan_skills_for_scope(&loaded, None)
                .expect("rescan Codex direct skills");
            assert_eq!(
                refreshed
                    .iter()
                    .find(|skill| skill.instance_id == "tool:codex:code-review")
                    .and_then(|skill| skill.enabled.get("codex"))
                    .copied(),
                Some(false)
            );
        });
    }

    #[test]
    fn project_skill_activation_targets_the_registered_repository() {
        with_temp_home(|home| {
            let repository = home.join("repo");
            let source = repository.join("skills").join("repo-skill");
            fs::create_dir_all(&source).expect("create repository skill");
            fs::write(source.join("SKILL.md"), "---\nname: repo-skill\n---\n")
                .expect("write repository skill");

            let mut config = AppConfig::default();
            config.tools.insert(
                "claude-code".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: home.join(".claude").join("skills"),
                    config_path: home.join(".claude"),
                },
            );
            config.projects.push(ProjectBinding {
                id: "repo".to_string(),
                name: "repo".to_string(),
                skills_dir: repository.join("skills"),
                root_path: Some(repository.clone()),
            });

            let skill = Skill::new(
                "repo-skill".to_string(),
                "repo-skill".to_string(),
                source.clone(),
            )
            .with_scope(
                SkillScope::Project,
                Some("repo".to_string()),
                Some("repo".to_string()),
            );

            apply_skill_tool_enabled_from_skills(
                &config,
                std::slice::from_ref(&skill),
                &skill.instance_id,
                "claude-code",
                true,
                Some(source.as_path()),
            )
            .expect("enable repository skill");

            let project_target = repository.join(".claude").join("skills").join("repo-skill");
            assert!(project_target.exists());
            assert!(!home
                .join(".claude")
                .join("skills")
                .join("repo-skill")
                .exists());

            apply_skill_tool_enabled_from_skills(
                &config,
                std::slice::from_ref(&skill),
                &skill.instance_id,
                "claude-code",
                false,
                Some(source.as_path()),
            )
            .expect("disable repository skill");
            assert!(source.exists());
            assert!(!project_target.exists());
        });
    }

    #[test]
    fn directly_installed_project_skill_can_toggle_with_disabled_suffix() {
        with_temp_home(|home| {
            let repository = home.join("direct-repo");
            let target_root = repository.join(".agents").join("skills");
            let source = target_root.join("direct-skill");
            fs::create_dir_all(&source).expect("create direct project skill");
            fs::write(source.join("SKILL.md"), "---\nname: direct-skill\n---\n")
                .expect("write direct project skill");

            let mut config = AppConfig::default();
            config.tools.insert(
                "vercel-skills".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: home.join(".agents").join("skills"),
                    config_path: home.join(".agents"),
                },
            );
            config.projects.push(ProjectBinding {
                id: "direct-repo".to_string(),
                name: "direct-repo".to_string(),
                skills_dir: repository.join("skills"),
                root_path: Some(repository),
            });

            let skill = Skill::new(
                "direct-skill".to_string(),
                "direct-skill".to_string(),
                source.clone(),
            )
            .with_scope(
                SkillScope::Project,
                Some("direct-repo".to_string()),
                Some("direct-repo".to_string()),
            );

            apply_skill_tool_enabled_from_skills(
                &config,
                std::slice::from_ref(&skill),
                &skill.instance_id,
                "vercel-skills",
                false,
                Some(source.as_path()),
            )
            .expect("disable direct project skill");

            let disabled_source = source.with_file_name("direct-skill.disabled-by-sm");
            assert!(!source.exists());
            assert!(disabled_source.exists());

            let disabled_skill = Skill::new(
                "direct-skill".to_string(),
                "direct-skill".to_string(),
                disabled_source.clone(),
            )
            .with_scope(
                SkillScope::Project,
                Some("direct-repo".to_string()),
                Some("direct-repo".to_string()),
            );
            apply_skill_tool_enabled_from_skills(
                &config,
                std::slice::from_ref(&disabled_skill),
                &disabled_skill.instance_id,
                "vercel-skills",
                true,
                Some(disabled_source.as_path()),
            )
            .expect("re-enable direct project skill");

            assert!(source.exists());
            assert!(!disabled_source.exists());
        });
    }
}
