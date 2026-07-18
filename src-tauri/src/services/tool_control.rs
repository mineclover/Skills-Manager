use std::path::Path;

use crate::models::AppConfig;
use crate::services::{ConfigManager, LinkerService};

/// Shared Tool activation use cases used by both the Tauri UI and CLI.
pub struct ToolControlService;

impl ToolControlService {
    pub fn set_enabled(tool_id: &str, enabled: bool) -> Result<(), String> {
        if !enabled {
            let manager = ConfigManager::new();
            let config = manager.load()?;
            let tool_config = config
                .get_tool_config(tool_id)
                .ok_or_else(|| format!("Tool not found: {tool_id}"))?;
            if should_remove_links_when_disabling_tool(&config) {
                remove_skill_links_for_tool(&config.skills_dir, &tool_config.skills_path, tool_id)?;
            }
        }

        Self::set_enabled_in_config(tool_id, enabled)
    }

    pub fn set_enabled_in_config(tool_id: &str, enabled: bool) -> Result<(), String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;

        if let Some(tool_config) = config.tools.get_mut(tool_id) {
            tool_config.enabled = enabled;
            return manager.save(&config);
        }

        if let Some(custom_tool) = config.custom_tools.get_mut(tool_id) {
            custom_tool.enabled = enabled;
            return manager.save(&config);
        }

        Err(format!("Tool not found: {tool_id}"))
    }
}

fn should_remove_links_when_disabling_tool(config: &AppConfig) -> bool {
    config
        .preferences
        .as_ref()
        .map(|preferences| preferences.remove_links_when_disabling_tool)
        .unwrap_or(false)
}

fn remove_skill_links_for_tool(
    hub_skills_dir: &Path,
    tool_skills_dir: &Path,
    tool_id: &str,
) -> Result<(), String> {
    if !hub_skills_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(hub_skills_dir)
        .map_err(|error| format!("Failed to read hub skills directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Failed to read skill entry: {error}"))?;
        let skill_path = entry.path();
        if !skill_path.is_dir() {
            continue;
        }

        let Some(skill_id) = skill_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if skill_id.starts_with('.') {
            continue;
        }

        LinkerService::disable_skill_for_tool(tool_skills_dir, skill_id, tool_id)?;
    }

    Ok(())
}
