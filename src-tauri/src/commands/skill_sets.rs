use crate::models::{
    AssignSkillSetReleaseRequest, CreateSkillSetBlueprintRequest, CreateSkillSetReleaseRequest,
    EffectiveSkillSet, ResolveEffectiveSkillSetRequest, ReviewSkillSetBlueprintRequest,
    SetSkillSetAssignmentActiveRequest, SetSkillSetAssignmentPriorityRequest,
    SkillSetActivationApplyResult, SkillSetActivationPlan, SkillSetDriftReport, SkillSetStore,
    UpdateSkillSetBlueprintRequest,
};
use crate::services::SkillSetService;

#[tauri::command]
pub fn get_skill_set_catalog() -> Result<SkillSetStore, String> {
    SkillSetService::catalog()
}

#[tauri::command]
pub fn create_skill_set_blueprint(
    request: CreateSkillSetBlueprintRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::create_blueprint(request)
}

#[tauri::command]
pub fn update_skill_set_blueprint(
    request: UpdateSkillSetBlueprintRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::update_blueprint(request)
}

#[tauri::command]
pub fn review_skill_set_blueprint(
    request: ReviewSkillSetBlueprintRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::review_blueprint(&request.blueprint_id)
}

#[tauri::command]
pub fn delete_skill_set_blueprint(blueprint_id: String) -> Result<SkillSetStore, String> {
    SkillSetService::delete_blueprint(&blueprint_id)
}

#[tauri::command]
pub fn create_skill_set_release(
    request: CreateSkillSetReleaseRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::create_release(request)
}

#[tauri::command]
pub fn assign_skill_set_release(
    request: AssignSkillSetReleaseRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::assign_release(request)
}

#[tauri::command]
pub fn set_skill_set_assignment_active(
    request: SetSkillSetAssignmentActiveRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::set_assignment_active(request)
}

#[tauri::command]
pub fn set_skill_set_assignment_priority(
    request: SetSkillSetAssignmentPriorityRequest,
) -> Result<SkillSetStore, String> {
    SkillSetService::set_assignment_priority(request)
}

#[tauri::command]
pub fn delete_skill_set_assignment(assignment_id: String) -> Result<SkillSetStore, String> {
    SkillSetService::delete_assignment(&assignment_id)
}

#[tauri::command]
pub fn resolve_effective_skill_set(
    request: ResolveEffectiveSkillSetRequest,
) -> Result<EffectiveSkillSet, String> {
    SkillSetService::resolve_effective_set(request)
}

#[tauri::command]
pub fn preview_skill_set_activation(
    assignment_id: String,
) -> Result<SkillSetActivationPlan, String> {
    SkillSetService::preview_activation(&assignment_id)
}

#[tauri::command]
pub fn get_skill_set_drift(assignment_id: String) -> Result<SkillSetDriftReport, String> {
    SkillSetService::inspect_drift(&assignment_id)
}

#[tauri::command]
pub fn apply_skill_set_activation(
    assignment_id: String,
    confirm_shared_root: Option<bool>,
) -> Result<SkillSetActivationApplyResult, String> {
    SkillSetService::apply_activation(&assignment_id, confirm_shared_root.unwrap_or(false))
}
