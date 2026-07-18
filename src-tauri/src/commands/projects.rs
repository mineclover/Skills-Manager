use crate::models::{AppConfig, ProjectBinding};
use crate::services::{AppCache, WorkspaceService};
use tauri::State;

#[tauri::command]
pub fn preview_project_binding(
    path: String,
    name: Option<String>,
) -> Result<ProjectBinding, String> {
    WorkspaceService::preview_project(&path, name.as_deref())
}

#[tauri::command]
pub fn register_project_binding(
    path: String,
    name: Option<String>,
    cache: State<AppCache>,
) -> Result<AppConfig, String> {
    let config = WorkspaceService::register_project(&path, name.as_deref())?;
    cache.invalidate_skills();
    Ok(config)
}

#[tauri::command]
pub fn set_active_project_binding(
    project_id: Option<String>,
    cache: State<AppCache>,
) -> Result<AppConfig, String> {
    let config = WorkspaceService::set_active_project(project_id.as_deref())?;
    cache.invalidate_skills();
    Ok(config)
}

#[tauri::command]
pub fn remove_project_binding(
    project_id: String,
    cache: State<AppCache>,
) -> Result<AppConfig, String> {
    let config = WorkspaceService::remove_project(&project_id)?;
    cache.invalidate_skills();
    Ok(config)
}
