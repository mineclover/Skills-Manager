use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{
    home_dir, ActivationPlanAction, ActivationProviderOutcome, AssignSkillSetReleaseRequest,
    CreateSkillSetBlueprintRequest, CreateSkillSetReleaseRequest, EffectiveSkillSet,
    EffectiveSkillSetMember, ResolveEffectiveSkillSetRequest, SetSkillSetAssignmentActiveRequest,
    SetSkillSetAssignmentPriorityRequest, SkillSetActivationApplyResult,
    SkillSetActivationOperation, SkillSetActivationPlan, SkillSetAssignment,
    SkillSetAssignmentRole, SkillSetBlueprint, SkillSetDriftReport, SkillSetMember,
    SkillSetMemberScopePolicy, SkillSetMemberSnapshot, SkillSetRelease, SkillSetStore,
    UpdateSkillSetBlueprintRequest,
};
use crate::services::{ConfigManager, ScannerService, SkillControlService, StudioFeedbackService};

const STORE_FILE_NAME: &str = "skill-sets.json";
const STORE_SCHEMA_VERSION: u32 = 1;

pub struct SkillSetService;

impl SkillSetService {
    fn store_path() -> PathBuf {
        home_dir()
            .unwrap_or_default()
            .join(".skills-manager")
            .join(STORE_FILE_NAME)
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs() as i64)
            .unwrap_or(0)
    }

    fn normalize_members(
        skill_ids: Vec<String>,
        member_scope_policies: std::collections::HashMap<String, SkillSetMemberScopePolicy>,
    ) -> Result<Vec<SkillSetMember>, String> {
        let mut members = Vec::new();
        for skill_id in skill_ids {
            let skill_id = skill_id.trim().to_string();
            if skill_id.is_empty() {
                continue;
            }
            if !members
                .iter()
                .any(|member: &SkillSetMember| member.skill_id == skill_id)
            {
                let scope_policy = member_scope_policies
                    .get(&skill_id)
                    .cloned()
                    .unwrap_or_default();
                members.push(SkillSetMember {
                    skill_id,
                    scope_policy,
                });
            }
        }
        if members.is_empty() {
            return Err("A skill set must include at least one canonical skill id".to_string());
        }
        Ok(members)
    }

    fn resolve_member<'a>(
        skills: &'a [crate::models::Skill],
        member: &SkillSetMember,
    ) -> Option<&'a crate::models::Skill> {
        let matching = |scope| {
            skills
                .iter()
                .find(|skill| skill.id == member.skill_id && skill.scope == scope)
        };
        match member.scope_policy {
            SkillSetMemberScopePolicy::Global => matching(crate::models::SkillScope::Global),
            SkillSetMemberScopePolicy::Project => matching(crate::models::SkillScope::Project),
            SkillSetMemberScopePolicy::ProjectThenGlobal => {
                matching(crate::models::SkillScope::Project)
                    .or_else(|| matching(crate::models::SkillScope::Global))
            }
            SkillSetMemberScopePolicy::ToolLocal => matching(crate::models::SkillScope::Tool),
        }
    }

    fn snapshot_members(members: &[SkillSetMember]) -> Result<Vec<SkillSetMemberSnapshot>, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_scoped_skills(&config)?;
        members
            .iter()
            .map(|member| {
                let skill = Self::resolve_member(&skills, member);
                let Some(skill) = skill else {
                    return Ok(SkillSetMemberSnapshot {
                        skill_id: member.skill_id.clone(),
                        scope_policy: member.scope_policy.clone(),
                        source_path: format!("unresolved:{}", member.skill_id),
                        scope: crate::models::SkillScope::Global,
                        contract_status: crate::models::SkillContractStatus::Unmanaged,
                        contract_digest: None,
                        purpose_summary: None,
                        evaluation_cases: Vec::new(),
                    });
                };
                let contract_digest = skill
                    .contract
                    .contract
                    .as_ref()
                    .map(|contract| {
                        serde_json::to_vec(contract)
                            .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
                            .map_err(|error| format!("Failed to snapshot contract: {error}"))
                    })
                    .transpose()?;
                Ok(SkillSetMemberSnapshot {
                    skill_id: member.skill_id.clone(),
                    scope_policy: member.scope_policy.clone(),
                    source_path: skill.path.to_string_lossy().to_string(),
                    scope: skill.scope.clone(),
                    contract_status: skill.contract.status.clone(),
                    contract_digest,
                    purpose_summary: skill
                        .contract
                        .contract
                        .as_ref()
                        .map(|contract| contract.purpose.summary.clone())
                        .filter(|summary| !summary.trim().is_empty()),
                    evaluation_cases: skill
                        .contract
                        .contract
                        .as_ref()
                        .map(|contract| contract.evaluation.cases.clone())
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    fn load() -> Result<SkillSetStore, String> {
        let path = Self::store_path();
        if !path.exists() {
            return Ok(SkillSetStore {
                schema_version: STORE_SCHEMA_VERSION,
                ..Default::default()
            });
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read skill set store: {error}"))?;
        let mut store: SkillSetStore = serde_json::from_str(&contents)
            .map_err(|error| format!("Failed to parse skill set store: {error}"))?;
        if store.schema_version == 0 {
            store.schema_version = STORE_SCHEMA_VERSION;
        }
        if store.schema_version != STORE_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported skill set store schema: {}",
                store.schema_version
            ));
        }
        Ok(store)
    }

    fn save(store: &SkillSetStore) -> Result<(), String> {
        let path = Self::store_path();
        let parent = path
            .parent()
            .ok_or_else(|| "Skill set store has no parent directory".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create skill set store directory: {error}"))?;
        let contents = serde_json::to_string_pretty(store)
            .map_err(|error| format!("Failed to serialize skill set store: {error}"))?;
        let temp = parent.join(format!(".{}.{}.tmp", STORE_FILE_NAME, Uuid::new_v4()));
        fs::write(&temp, contents)
            .map_err(|error| format!("Failed to write skill set store: {error}"))?;
        fs::rename(&temp, &path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            format!("Failed to replace skill set store: {error}")
        })
    }

    pub fn catalog() -> Result<SkillSetStore, String> {
        Self::load()
    }

    pub fn create_blueprint(
        request: CreateSkillSetBlueprintRequest,
    ) -> Result<SkillSetStore, String> {
        let name = request.name.trim().to_string();
        if name.is_empty() {
            return Err("Skill set name is required".to_string());
        }
        let mut store = Self::load()?;
        if store
            .blueprints
            .iter()
            .any(|blueprint| blueprint.name.eq_ignore_ascii_case(&name))
        {
            return Err(format!("A skill set named '{name}' already exists"));
        }
        let now = Self::now();
        store.blueprints.push(SkillSetBlueprint {
            id: format!("set-{}", Uuid::new_v4()),
            name,
            description: request.description.trim().to_string(),
            members: Self::normalize_members(request.skill_ids, request.member_scope_policies)?,
            created_at: now,
            updated_at: now,
            reviewed_at: None,
        });
        Self::save(&store)?;
        Ok(store)
    }

    pub fn update_blueprint(
        request: UpdateSkillSetBlueprintRequest,
    ) -> Result<SkillSetStore, String> {
        let name = request.name.trim().to_string();
        if name.is_empty() {
            return Err("Skill set name is required".to_string());
        }
        let members = Self::normalize_members(request.skill_ids, request.member_scope_policies)?;
        let mut store = Self::load()?;
        if store.blueprints.iter().any(|blueprint| {
            blueprint.id != request.blueprint_id && blueprint.name.eq_ignore_ascii_case(&name)
        }) {
            return Err(format!("A skill set named '{name}' already exists"));
        }
        let blueprint = store
            .blueprints
            .iter_mut()
            .find(|blueprint| blueprint.id == request.blueprint_id)
            .ok_or_else(|| format!("Skill set not found: {}", request.blueprint_id))?;
        blueprint.name = name;
        blueprint.description = request.description.trim().to_string();
        blueprint.members = members;
        blueprint.updated_at = Self::now();
        blueprint.reviewed_at = None;
        Self::save(&store)?;
        Ok(store)
    }

    pub fn review_blueprint(blueprint_id: &str) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        let blueprint = store
            .blueprints
            .iter_mut()
            .find(|blueprint| blueprint.id == blueprint_id)
            .ok_or_else(|| format!("Skill set not found: {blueprint_id}"))?;
        blueprint.reviewed_at = Some(Self::now());
        Self::save(&store)?;
        Ok(store)
    }

    pub fn delete_blueprint(blueprint_id: &str) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        if store
            .releases
            .iter()
            .any(|release| release.blueprint_id == blueprint_id)
        {
            return Err(
                "Skill sets with releases cannot be deleted; preserve release history instead"
                    .to_string(),
            );
        }
        let previous_len = store.blueprints.len();
        store
            .blueprints
            .retain(|blueprint| blueprint.id != blueprint_id);
        if previous_len == store.blueprints.len() {
            return Err(format!("Skill set not found: {blueprint_id}"));
        }
        Self::save(&store)?;
        Ok(store)
    }

    pub fn create_release(request: CreateSkillSetReleaseRequest) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        let blueprint = store
            .blueprints
            .iter()
            .find(|blueprint| blueprint.id == request.blueprint_id)
            .cloned()
            .ok_or_else(|| format!("Skill set not found: {}", request.blueprint_id))?;
        if blueprint.reviewed_at.is_none() {
            return Err("Review the blueprint before creating a release".to_string());
        }
        let member_snapshots = Self::snapshot_members(&blueprint.members)?;
        let created_at = Self::now();
        let label = request.label.trim().to_string();
        let release_notes = request.release_notes.trim().to_string();
        let digest_input =
            serde_json::to_vec(&(&blueprint.id, &label, &release_notes, &blueprint.members))
                .map_err(|error| format!("Failed to digest release: {error}"))?;
        let content_digest = format!("{:x}", Sha256::digest(digest_input));
        store.releases.push(SkillSetRelease {
            id: format!("release-{}", Uuid::new_v4()),
            blueprint_id: blueprint.id,
            blueprint_name: blueprint.name,
            label,
            release_notes,
            content_digest,
            members: blueprint.members,
            member_snapshots,
            created_at,
        });
        Self::save(&store)?;
        Ok(store)
    }

    pub fn assign_release(request: AssignSkillSetReleaseRequest) -> Result<SkillSetStore, String> {
        let work_scope = request.work_scope.trim().to_string();
        if work_scope.is_empty() && request.role == SkillSetAssignmentRole::Recommended {
            return Err("Work scope is required".to_string());
        }
        let work_scope = if work_scope.is_empty() {
            "default".to_string()
        } else {
            work_scope
        };
        let mut store = Self::load()?;
        if !store
            .releases
            .iter()
            .any(|release| release.id == request.release_id)
        {
            return Err(format!(
                "Skill set release not found: {}",
                request.release_id
            ));
        }
        let mut provider_ids = request
            .provider_ids
            .into_iter()
            .map(|provider_id| provider_id.trim().to_string())
            .filter(|provider_id| !provider_id.is_empty())
            .collect::<Vec<_>>();
        provider_ids.sort();
        provider_ids.dedup();
        let now = Self::now();
        store.assignments.push(SkillSetAssignment {
            id: format!("assignment-{}", Uuid::new_v4()),
            release_id: request.release_id,
            project_id: request
                .project_id
                .filter(|project_id| !project_id.trim().is_empty()),
            work_scope,
            role: request.role,
            provider_ids,
            priority: request.priority,
            active: true,
            created_at: now,
            updated_at: now,
        });
        Self::save(&store)?;
        Ok(store)
    }

    pub fn set_assignment_active(
        request: SetSkillSetAssignmentActiveRequest,
    ) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        let assignment = store
            .assignments
            .iter_mut()
            .find(|assignment| assignment.id == request.assignment_id)
            .ok_or_else(|| format!("Skill set assignment not found: {}", request.assignment_id))?;
        assignment.active = request.active;
        assignment.updated_at = Self::now();
        Self::save(&store)?;
        Ok(store)
    }

    pub fn set_assignment_priority(
        request: SetSkillSetAssignmentPriorityRequest,
    ) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        let assignment = store
            .assignments
            .iter_mut()
            .find(|assignment| assignment.id == request.assignment_id)
            .ok_or_else(|| format!("Skill set assignment not found: {}", request.assignment_id))?;
        assignment.priority = request.priority;
        assignment.updated_at = Self::now();
        Self::save(&store)?;
        Ok(store)
    }

    pub fn delete_assignment(assignment_id: &str) -> Result<SkillSetStore, String> {
        let mut store = Self::load()?;
        let previous_len = store.assignments.len();
        store
            .assignments
            .retain(|assignment| assignment.id != assignment_id);
        if previous_len == store.assignments.len() {
            return Err(format!("Skill set assignment not found: {assignment_id}"));
        }
        Self::save(&store)?;
        Ok(store)
    }

    pub fn resolve_effective_set(
        request: ResolveEffectiveSkillSetRequest,
    ) -> Result<EffectiveSkillSet, String> {
        let work_scope = request.work_scope.trim().to_string();
        if work_scope.is_empty() {
            return Err("Work scope is required to resolve an effective skill set".to_string());
        }
        let project_id = request.project_id.filter(|value| !value.trim().is_empty());
        let store = Self::load()?;
        let mut assignments = store
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.active
                    && (assignment.role == SkillSetAssignmentRole::Default
                        || assignment.work_scope == work_scope)
                    && (assignment.project_id.is_none() || assignment.project_id == project_id)
            })
            .collect::<Vec<_>>();
        assignments.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| right.project_id.is_some().cmp(&left.project_id.is_some()))
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id.as_deref())?;
        let mut by_skill = std::collections::BTreeMap::<String, EffectiveSkillSetMember>::new();
        let mut unresolved_skill_ids = Vec::new();
        let mut release_ids = Vec::new();
        for assignment in &assignments {
            let release = store
                .releases
                .iter()
                .find(|release| release.id == assignment.release_id)
                .ok_or_else(|| {
                    format!(
                        "Assignment references missing release: {}",
                        assignment.release_id
                    )
                })?;
            release_ids.push(release.id.clone());
            for member in &release.members {
                let instance_id =
                    Self::resolve_member(&skills, member).map(|skill| skill.instance_id.clone());
                let unresolved_key =
                    format!("{} ({:?})", member.skill_id, member.scope_policy).to_lowercase();
                if instance_id.is_none() && !unresolved_skill_ids.contains(&unresolved_key) {
                    unresolved_skill_ids.push(unresolved_key);
                }
                let entry_key = format!("{}:{:?}", member.skill_id, member.scope_policy);
                let entry = by_skill
                    .entry(entry_key)
                    .or_insert_with(|| EffectiveSkillSetMember {
                        skill_id: member.skill_id.clone(),
                        scope_policy: member.scope_policy.clone(),
                        skill_instance_id: instance_id.clone(),
                        included_by_release_ids: Vec::new(),
                    });
                if entry.skill_instance_id.is_none() {
                    entry.skill_instance_id = instance_id;
                }
                entry.included_by_release_ids.push(release.id.clone());
            }
        }
        Ok(EffectiveSkillSet {
            project_id,
            work_scope,
            assignment_ids: assignments
                .into_iter()
                .map(|assignment| assignment.id.clone())
                .collect(),
            release_ids,
            members: by_skill.into_values().collect(),
            unresolved_skill_ids,
            generated_at: Self::now(),
        })
    }

    pub fn preview_activation(assignment_id: &str) -> Result<SkillSetActivationPlan, String> {
        let store = Self::load()?;
        let assignment = store
            .assignments
            .iter()
            .find(|item| item.id == assignment_id)
            .ok_or_else(|| format!("Skill set assignment not found: {assignment_id}"))?;
        if !assignment.active {
            return Err("Activate the assignment before generating an activation plan".to_string());
        }
        if assignment.provider_ids.is_empty() {
            return Err(
                "An assignment must target at least one configured tool provider".to_string(),
            );
        }
        let release = store
            .releases
            .iter()
            .find(|item| item.id == assignment.release_id)
            .ok_or_else(|| format!("Skill set release not found: {}", assignment.release_id))?;
        let config = ConfigManager::new().load()?;
        let known_tools = config.collect_tool_configs();
        let skills =
            ScannerService::scan_skills_for_scope(&config, assignment.project_id.as_deref())?;
        let mut operations = Vec::new();
        let mut missing_skill_ids = Vec::new();
        for member in &release.members {
            let Some(skill) = Self::resolve_member(&skills, member) else {
                missing_skill_ids.push(
                    format!("{} ({:?})", member.skill_id, member.scope_policy).to_lowercase(),
                );
                continue;
            };
            for tool_id in &assignment.provider_ids {
                if !known_tools.iter().any(|(id, _)| id == tool_id) {
                    return Err(format!("Unknown tool provider: {tool_id}"));
                }
                let current_enabled = skill.is_enabled_for(tool_id);
                operations.push(SkillSetActivationOperation {
                    skill_id: member.skill_id.clone(),
                    skill_instance_id: skill.instance_id.clone(),
                    tool_id: tool_id.clone(),
                    current_enabled,
                    action: if current_enabled {
                        ActivationPlanAction::Unchanged
                    } else {
                        ActivationPlanAction::Enable
                    },
                    reason: if current_enabled {
                        "Already enabled for this provider".to_string()
                    } else {
                        "Required by the assigned skill set release".to_string()
                    },
                });
            }
        }
        Ok(SkillSetActivationPlan {
            assignment_id: assignment.id.clone(),
            release_id: release.id.clone(),
            project_id: assignment.project_id.clone(),
            work_scope: assignment.work_scope.clone(),
            operations,
            missing_skill_ids,
            generated_at: Self::now(),
        })
    }

    pub fn inspect_drift(assignment_id: &str) -> Result<SkillSetDriftReport, String> {
        let plan = Self::preview_activation(assignment_id)?;
        let disabled_operations = plan
            .operations
            .iter()
            .filter(|operation| operation.action == ActivationPlanAction::Enable)
            .cloned()
            .collect::<Vec<_>>();
        Ok(SkillSetDriftReport {
            assignment_id: plan.assignment_id,
            release_id: plan.release_id,
            project_id: plan.project_id,
            work_scope: plan.work_scope,
            compliant: disabled_operations.is_empty() && plan.missing_skill_ids.is_empty(),
            disabled_operations,
            missing_skill_ids: plan.missing_skill_ids,
            generated_at: Self::now(),
        })
    }

    pub fn apply_activation(assignment_id: &str) -> Result<SkillSetActivationApplyResult, String> {
        let plan = Self::preview_activation(assignment_id)?;
        if !plan.missing_skill_ids.is_empty() {
            return Err(format!(
                "Cannot apply activation while release members are missing: {}",
                plan.missing_skill_ids.join(", ")
            ));
        }
        let mut result = SkillSetActivationApplyResult {
            plan: plan.clone(),
            activation_run_id: String::new(),
            applied_count: 0,
            skipped_count: 0,
            failed_count: 0,
            failures: Vec::new(),
            provider_outcomes: Vec::new(),
        };
        let mut outcomes = BTreeMap::<String, ActivationProviderOutcome>::new();
        for operation in &plan.operations {
            let outcome = outcomes
                .entry(operation.tool_id.clone())
                .or_insert_with(|| ActivationProviderOutcome {
                    provider_id: operation.tool_id.clone(),
                    applied_count: 0,
                    skipped_count: 0,
                    failed_count: 0,
                });
            if operation.action == ActivationPlanAction::Unchanged {
                result.skipped_count += 1;
                outcome.skipped_count += 1;
                continue;
            }
            match SkillControlService::set_skill_enabled_for_scope(
                plan.project_id.as_deref(),
                &operation.skill_instance_id,
                &operation.tool_id,
                true,
            ) {
                Ok(report) if report.failed_count == 0 => {
                    result.applied_count += report.applied_count;
                    outcome.applied_count += report.applied_count;
                }
                Ok(report) => {
                    let failures = report.failed_count.max(1);
                    result.failed_count += failures;
                    outcome.failed_count += failures;
                    result
                        .failures
                        .extend(report.failures.into_iter().map(|failure| failure.message));
                }
                Err(error) => {
                    result.failed_count += 1;
                    outcome.failed_count += 1;
                    result.failures.push(error);
                }
            }
        }
        result.provider_outcomes = outcomes.into_values().collect();
        let run = StudioFeedbackService::record_activation_run(
            &plan.assignment_id,
            &plan.release_id,
            plan.project_id.clone(),
            &plan.work_scope,
            result.applied_count,
            result.skipped_count,
            result.failed_count,
            result.provider_outcomes.clone(),
        )?;
        result.activation_run_id = run.id;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProjectBinding;
    use crate::test_support::with_temp_home;
    use std::collections::HashMap;

    #[test]
    fn release_is_immutable_snapshot_of_blueprint_members() {
        with_temp_home(|_| {
            let created = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Integration".to_string(),
                description: String::new(),
                skill_ids: vec!["upstream".to_string(), "testing".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();
            let blueprint_id = created.blueprints[0].id.clone();
            SkillSetService::review_blueprint(&blueprint_id).unwrap();
            let released = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: blueprint_id.clone(),
                label: "v1".to_string(),
                release_notes: "Initial reviewed snapshot".to_string(),
            })
            .unwrap();
            let release = released.releases[0].clone();

            SkillSetService::update_blueprint(UpdateSkillSetBlueprintRequest {
                blueprint_id,
                name: "Integration".to_string(),
                description: String::new(),
                skill_ids: vec!["upstream".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();

            assert_eq!(release.members.len(), 2);
            assert_eq!(release.member_snapshots.len(), 2);
            assert_eq!(
                release.member_snapshots[0].source_path,
                "unresolved:upstream"
            );
            assert!(!release.content_digest.is_empty());
        });
    }

    #[test]
    fn member_scope_policy_is_preserved_and_defaults_safely() {
        let mut policies = HashMap::new();
        policies.insert(
            "project-only".to_string(),
            SkillSetMemberScopePolicy::Project,
        );
        let members = SkillSetService::normalize_members(
            vec!["project-only".to_string(), "fallback".to_string()],
            policies,
        )
        .unwrap();
        assert_eq!(members[0].scope_policy, SkillSetMemberScopePolicy::Project);
        assert_eq!(
            members[1].scope_policy,
            SkillSetMemberScopePolicy::ProjectThenGlobal
        );
    }

    #[test]
    fn assignment_can_be_deactivated_without_mutating_release() {
        with_temp_home(|_| {
            let store = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Review".to_string(),
                description: String::new(),
                skill_ids: vec!["review".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();
            SkillSetService::review_blueprint(&store.blueprints[0].id).unwrap();
            let store = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: store.blueprints[0].id.clone(),
                label: String::new(),
                release_notes: String::new(),
            })
            .unwrap();
            let store = SkillSetService::assign_release(AssignSkillSetReleaseRequest {
                release_id: store.releases[0].id.clone(),
                project_id: Some("project-a".to_string()),
                work_scope: "code-review".to_string(),
                role: SkillSetAssignmentRole::Recommended,
                provider_ids: vec!["codex".to_string(), "codex".to_string()],
                priority: 0,
            })
            .unwrap();
            let assignment_id = store.assignments[0].id.clone();
            let updated =
                SkillSetService::set_assignment_active(SetSkillSetAssignmentActiveRequest {
                    assignment_id,
                    active: false,
                })
                .unwrap();

            assert!(!updated.assignments[0].active);
            assert_eq!(updated.releases.len(), 1);
            assert_eq!(updated.assignments[0].provider_ids, vec!["codex"]);
        });
    }

    #[test]
    fn release_requires_explicit_human_review() {
        with_temp_home(|_| {
            let store = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Draft".to_string(),
                description: String::new(),
                skill_ids: vec!["draft-skill".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();
            assert!(
                SkillSetService::create_release(CreateSkillSetReleaseRequest {
                    blueprint_id: store.blueprints[0].id.clone(),
                    label: String::new(),
                    release_notes: String::new(),
                })
                .is_err()
            );
        });
    }

    #[test]
    fn effective_set_merges_global_and_project_overlays_without_duplicate_members() {
        with_temp_home(|home| {
            let project_root = home.join("project-a");
            let project_skills = project_root.join(".claude").join("skills");
            fs::create_dir_all(&project_skills).unwrap();
            let mut config = ConfigManager::new().load().unwrap();
            config.projects.push(ProjectBinding {
                id: "project-a".to_string(),
                name: "Project A".to_string(),
                skills_dir: project_skills,
                root_path: Some(project_root),
            });
            ConfigManager::new().save(&config).unwrap();

            let store = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Global overlay".to_string(),
                description: String::new(),
                skill_ids: vec!["shared".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();
            SkillSetService::review_blueprint(&store.blueprints[0].id).unwrap();
            let store = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: store.blueprints[0].id.clone(),
                label: String::new(),
                release_notes: String::new(),
            })
            .unwrap();
            let global_release_id = store.releases[0].id.clone();
            let store = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Project overlay".to_string(),
                description: String::new(),
                skill_ids: vec!["shared".to_string(), "project-only".to_string()],
                member_scope_policies: Default::default(),
            })
            .unwrap();
            let project_blueprint_id = store
                .blueprints
                .iter()
                .find(|item| item.name == "Project overlay")
                .unwrap()
                .id
                .clone();
            SkillSetService::review_blueprint(&project_blueprint_id).unwrap();
            let store = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: project_blueprint_id,
                label: String::new(),
                release_notes: String::new(),
            })
            .unwrap();
            let project_release_id = store.releases.last().unwrap().id.clone();
            SkillSetService::assign_release(AssignSkillSetReleaseRequest {
                release_id: global_release_id.clone(),
                project_id: None,
                work_scope: "integration".to_string(),
                role: SkillSetAssignmentRole::Default,
                provider_ids: vec![],
                priority: 0,
            })
            .unwrap();
            SkillSetService::assign_release(AssignSkillSetReleaseRequest {
                release_id: project_release_id.clone(),
                project_id: Some("project-a".to_string()),
                work_scope: "integration".to_string(),
                role: SkillSetAssignmentRole::Recommended,
                provider_ids: vec![],
                priority: 10,
            })
            .unwrap();

            let effective =
                SkillSetService::resolve_effective_set(ResolveEffectiveSkillSetRequest {
                    project_id: Some("project-a".to_string()),
                    work_scope: "integration".to_string(),
                })
                .unwrap();
            assert_eq!(effective.assignment_ids.len(), 2);
            assert_eq!(
                effective.release_ids,
                vec![project_release_id, global_release_id.clone()]
            );
            assert_eq!(effective.members.len(), 2);
            assert_eq!(
                effective
                    .members
                    .iter()
                    .find(|item| item.skill_id == "shared")
                    .unwrap()
                    .included_by_release_ids
                    .len(),
                2
            );
            assert_eq!(effective.unresolved_skill_ids.len(), 2);

            let baseline =
                SkillSetService::resolve_effective_set(ResolveEffectiveSkillSetRequest {
                    project_id: Some("project-a".to_string()),
                    work_scope: "deployment".to_string(),
                })
                .unwrap();
            assert_eq!(baseline.release_ids, vec![global_release_id]);
            assert_eq!(baseline.members.len(), 1);
        });
    }
}
