use std::path::{Path, PathBuf};

use crate::models::{AppConfig, Skill};
use crate::services::{
    skill_is_direct_tool_install, skill_tool_skills_dir, LinkerService, LinkStatus, ScannerService,
};

pub fn resolve_skill_source_path(_config: &AppConfig, skill: &Skill) -> PathBuf {
    skill.path.clone()
}

pub fn load_skill_by_instance_id(config: &AppConfig, instance_id: &str) -> Result<Skill, String> {
    ScannerService::scan_scoped_skills(config)?
        .into_iter()
        .find(|item| item.instance_id == instance_id)
        .ok_or_else(|| format!("Skill not found: {}", instance_id))
}

pub fn apply_skill_tool_enabled(
    config: &AppConfig,
    instance_id: &str,
    tool_id: &str,
    enabled: bool,
    skill_path: Option<&Path>,
) -> Result<(), String> {
    let tool_config = config
        .get_tool_config(tool_id)
        .ok_or_else(|| format!("Tool not found: {}", tool_id))?;

    if !tool_config.enabled {
        return Err(format!("Tool is disabled: {}", tool_id));
    }

    if enabled {
        let skill = load_skill_by_instance_id(config, instance_id)?;
        let tool_skills_dir = skill_tool_skills_dir(config, &skill, tool_id)?;
        let skill_path = match skill_path {
            Some(path) => path.to_path_buf(),
            None => resolve_skill_source_path(config, &skill),
        };
        if !skill_path.exists() {
            return Err(format!("Skill not found: {}", instance_id));
        }

        if skill_is_direct_tool_install(&skill, &tool_skills_dir) {
            return Ok(());
        }

        return LinkerService::enable_skill_for_tool(
            &skill_path,
            &tool_skills_dir,
            &skill.id,
            tool_id,
        );
    }

    let skill = load_skill_by_instance_id(config, instance_id)?;
    let tool_skills_dir = skill_tool_skills_dir(config, &skill, tool_id)?;
    if skill_is_direct_tool_install(&skill, &tool_skills_dir) {
        return Err(format!(
            "Cannot disable a project Skill stored directly in the tool directory: {}",
            skill.path.display()
        ));
    }
    match LinkerService::check_link_for_scoped_skill(
        &skill.path,
        &tool_skills_dir,
        &skill.id,
        tool_id,
        &skill.scope,
    ) {
        LinkStatus::Valid => LinkerService::disable_skill_for_tool(&tool_skills_dir, &skill.id, tool_id),
        LinkStatus::Missing => Ok(()),
        _ => Err(format!(
            "Skill target belongs to another instance: {}",
            instance_id
        )),
    }
}
