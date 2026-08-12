use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{
    home_dir, ActivationPlanAction, AssignSkillSetReleaseRequest, CreateSkillSetBlueprintRequest,
    CreateSkillSetReleaseRequest, SetSkillSetAssignmentActiveRequest,
    SkillSetActivationApplyResult, SkillSetActivationOperation, SkillSetActivationPlan,
    SkillSetAssignment, SkillSetBlueprint, SkillSetMember, SkillSetRelease, SkillSetStore,
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

    fn normalize_members(skill_ids: Vec<String>) -> Result<Vec<SkillSetMember>, String> {
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
                members.push(SkillSetMember { skill_id });
            }
        }
        if members.is_empty() {
            return Err("A skill set must include at least one canonical skill id".to_string());
        }
        Ok(members)
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
            members: Self::normalize_members(request.skill_ids)?,
            created_at: now,
            updated_at: now,
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
        let members = Self::normalize_members(request.skill_ids)?;
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
        let created_at = Self::now();
        let label = request.label.trim().to_string();
        let digest_input = serde_json::to_vec(&(&blueprint.id, &label, &blueprint.members))
            .map_err(|error| format!("Failed to digest release: {error}"))?;
        let content_digest = format!("{:x}", Sha256::digest(digest_input));
        store.releases.push(SkillSetRelease {
            id: format!("release-{}", Uuid::new_v4()),
            blueprint_id: blueprint.id,
            blueprint_name: blueprint.name,
            label,
            content_digest,
            members: blueprint.members,
            created_at,
        });
        Self::save(&store)?;
        Ok(store)
    }

    pub fn assign_release(request: AssignSkillSetReleaseRequest) -> Result<SkillSetStore, String> {
        let work_scope = request.work_scope.trim().to_string();
        if work_scope.is_empty() {
            return Err("Work scope is required".to_string());
        }
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
            provider_ids,
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
            let Some(skill) = skills.iter().find(|skill| skill.id == member.skill_id) else {
                missing_skill_ids.push(member.skill_id.clone());
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
        };
        for operation in &plan.operations {
            if operation.action == ActivationPlanAction::Unchanged {
                result.skipped_count += 1;
                continue;
            }
            match SkillControlService::set_skill_enabled_for_scope(
                plan.project_id.as_deref(),
                &operation.skill_instance_id,
                &operation.tool_id,
                true,
            ) {
                Ok(report) if report.failed_count == 0 => {
                    result.applied_count += report.applied_count
                }
                Ok(report) => {
                    result.failed_count += report.failed_count.max(1);
                    result
                        .failures
                        .extend(report.failures.into_iter().map(|failure| failure.message));
                }
                Err(error) => {
                    result.failed_count += 1;
                    result.failures.push(error);
                }
            }
        }
        let run = StudioFeedbackService::record_activation_run(
            &plan.assignment_id,
            &plan.release_id,
            plan.project_id.clone(),
            &plan.work_scope,
            result.applied_count,
            result.skipped_count,
            result.failed_count,
        )?;
        result.activation_run_id = run.id;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_home;

    #[test]
    fn release_is_immutable_snapshot_of_blueprint_members() {
        with_temp_home(|_| {
            let created = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Integration".to_string(),
                description: String::new(),
                skill_ids: vec!["upstream".to_string(), "testing".to_string()],
            })
            .unwrap();
            let blueprint_id = created.blueprints[0].id.clone();
            let released = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: blueprint_id.clone(),
                label: "v1".to_string(),
            })
            .unwrap();
            let release = released.releases[0].clone();

            SkillSetService::update_blueprint(UpdateSkillSetBlueprintRequest {
                blueprint_id,
                name: "Integration".to_string(),
                description: String::new(),
                skill_ids: vec!["upstream".to_string()],
            })
            .unwrap();

            assert_eq!(release.members.len(), 2);
            assert!(!release.content_digest.is_empty());
        });
    }

    #[test]
    fn assignment_can_be_deactivated_without_mutating_release() {
        with_temp_home(|_| {
            let store = SkillSetService::create_blueprint(CreateSkillSetBlueprintRequest {
                name: "Review".to_string(),
                description: String::new(),
                skill_ids: vec!["review".to_string()],
            })
            .unwrap();
            let store = SkillSetService::create_release(CreateSkillSetReleaseRequest {
                blueprint_id: store.blueprints[0].id.clone(),
                label: String::new(),
            })
            .unwrap();
            let store = SkillSetService::assign_release(AssignSkillSetReleaseRequest {
                release_id: store.releases[0].id.clone(),
                project_id: Some("project-a".to_string()),
                work_scope: "code-review".to_string(),
                provider_ids: vec!["codex".to_string(), "codex".to_string()],
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
}
