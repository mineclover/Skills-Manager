use crate::models::{SkillBinding, SkillOperationPreview, SkillProviderInventory};
use crate::services::ProviderInventoryService;

#[tauri::command]
pub fn list_skill_providers(project_id: Option<String>) -> Result<SkillProviderInventory, String> {
    ProviderInventoryService::list(project_id.as_deref())
}

#[tauri::command]
pub fn list_skill_bindings(
    project_id: Option<String>,
    provider_id: Option<String>,
    skill_instance_id: Option<String>,
) -> Result<Vec<SkillBinding>, String> {
    ProviderInventoryService::list_bindings(
        project_id.as_deref(),
        provider_id.as_deref(),
        skill_instance_id.as_deref(),
    )
}

#[tauri::command]
pub fn preview_skill_operation(
    project_id: Option<String>,
    skill_instance_id: String,
    provider_id: String,
    enabled: bool,
) -> Result<SkillOperationPreview, String> {
    ProviderInventoryService::preview_binding_operation(
        project_id.as_deref(),
        &skill_instance_id,
        &provider_id,
        enabled,
    )
}
