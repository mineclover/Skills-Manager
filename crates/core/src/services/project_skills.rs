use std::path::{Path, PathBuf};

use crate::models::{AppConfig, ProjectBinding, Skill, SkillScope, SUPPORTED_TOOLS};

pub const PROJECT_SKILL_STORE_RELATIVE_DIR: &str = ".skills-manager/skills";

pub fn managed_project_skills_dir(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_SKILL_STORE_RELATIVE_DIR)
}

pub fn project_tool_relative_skills_dir(tool_id: &str) -> Option<&'static str> {
    let normalized_tool_id = match tool_id {
        // Older test/config fixtures used this pre-rename identifier.
        "claude" => "claude-code",
        other => other,
    };
    SUPPORTED_TOOLS
        .iter()
        .find(|definition| definition.id == normalized_tool_id)
        .and_then(|definition| definition.project_skills_dir())
}

pub fn project_tool_skills_dir(project: &ProjectBinding, tool_id: &str) -> Result<PathBuf, String> {
    let root = project.root_path.as_deref().ok_or_else(|| {
        format!(
            "project binding '{}' must be rebound to a project root before installing tool-specific Skills",
            project.name
        )
    })?;
    let relative = project_tool_relative_skills_dir(tool_id)
        .ok_or_else(|| format!("tool does not support managed project Skills: {tool_id}"))?;
    Ok(root.join(relative))
}

pub fn skill_tool_skills_dir(
    config: &AppConfig,
    skill: &Skill,
    tool_id: &str,
) -> Result<PathBuf, String> {
    if skill.scope == SkillScope::Global {
        return config
            .get_tool_config(tool_id)
            .map(|tool| tool.skills_path)
            .ok_or_else(|| format!("Tool not found: {tool_id}"));
    }

    let project_id = skill
        .project_id
        .as_deref()
        .ok_or_else(|| format!("project Skill is missing project_id: {}", skill.instance_id))?;
    let project = config
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| format!("Project binding not found: {project_id}"))?;
    project_tool_skills_dir(project, tool_id)
}

pub fn skill_is_direct_tool_install(skill: &Skill, tool_skills_dir: &Path) -> bool {
    skill.path == tool_skills_dir.join(&skill.id)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::models::ProjectBinding;

    use super::{managed_project_skills_dir, project_tool_skills_dir};

    fn project() -> ProjectBinding {
        ProjectBinding {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            root_path: Some(PathBuf::from("/work/alpha")),
            skills_dir: PathBuf::from("/work/alpha/.skills-manager/skills"),
        }
    }

    #[test]
    fn managed_store_is_separate_from_tool_directories() {
        assert_eq!(
            managed_project_skills_dir(PathBuf::from("/work/alpha").as_path()),
            PathBuf::from("/work/alpha/.skills-manager/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "claude-code").unwrap(),
            PathBuf::from("/work/alpha/.claude/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "codex").unwrap(),
            PathBuf::from("/work/alpha/.agents/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "vercel-skills").unwrap(),
            PathBuf::from("/work/alpha/.agents/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "opencode").unwrap(),
            PathBuf::from("/work/alpha/.opencode/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "cursor").unwrap(),
            PathBuf::from("/work/alpha/.cursor/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "gemini").unwrap(),
            PathBuf::from("/work/alpha/.gemini/skills")
        );
        assert_eq!(
            project_tool_skills_dir(&project(), "deepseek-harness").unwrap(),
            PathBuf::from("/work/alpha/.dsh/skills")
        );
    }

    #[test]
    fn unsupported_tools_are_rejected_instead_of_guessing_a_path() {
        let error = project_tool_skills_dir(&project(), "unknown-tool").unwrap_err();
        assert!(error.contains("does not support managed project Skills"));
    }
}
