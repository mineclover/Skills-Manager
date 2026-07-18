use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::models::{AppConfig, ProjectBinding, SUPPORTED_TOOLS};
use crate::services::ConfigManager;

/// Repository/workspace discovery compatible with the `skills` CLI layout.
///
/// The manager keeps the selected repository as a ProjectBinding for backwards
/// compatibility, while this service derives the conventional skill roots from
/// the repository instead of requiring one agent-specific directory.
pub struct WorkspaceService;

impl WorkspaceService {
    pub fn discover_skill_roots(root_path: &Path) -> Vec<PathBuf> {
        if is_skill_dir(root_path) {
            return vec![root_path.to_path_buf()];
        }

        let mut candidates = vec![
            root_path.join("skills"),
            root_path.join(".agents").join("skills"),
            root_path.join(".claude").join("skills"),
            root_path.join(".codex").join("skills"),
        ];

        for definition in SUPPORTED_TOOLS {
            candidates.push(root_path.join(definition.config_dir).join("skills"));
            for alternate in definition.alt_config_dirs {
                candidates.push(root_path.join(alternate).join("skills"));
            }
        }

        let mut roots = Vec::new();
        for candidate in candidates {
            if candidate.is_dir() && !roots.iter().any(|root| root == &candidate) {
                roots.push(candidate);
            }
        }
        roots
    }

    pub fn build_project_binding(
        path: &Path,
        project_name: Option<&str>,
    ) -> Result<ProjectBinding, String> {
        let normalized_path = normalize_existing_directory(path)?;
        let repository_root = if Self::is_repository_root(&normalized_path) {
            Some(normalized_path.clone())
        } else {
            None
        };
        let roots = repository_root
            .as_deref()
            .map(|root| {
                let discovered = Self::discover_skill_roots(root);
                if discovered.is_empty() {
                    vec![root.join("skills")]
                } else {
                    discovered
                }
            })
            .unwrap_or_else(|| vec![normalized_path.clone()]);
        let skills_dir = roots
            .first()
            .cloned()
            .unwrap_or_else(|| normalized_path.clone());
        let default_name = default_project_name(repository_root.as_deref().unwrap_or(&skills_dir));
        let name = project_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&default_name)
            .to_string();
        let id = project_id(repository_root.as_deref().unwrap_or(&skills_dir));

        Ok(ProjectBinding {
            id,
            name,
            skills_dir,
            root_path: repository_root,
        })
    }

    pub fn is_repository_root(path: &Path) -> bool {
        if is_skill_dir(path) {
            return false;
        }
        path.join(".git").exists() || !Self::discover_skill_roots(path).is_empty()
    }

    pub fn project_skill_roots(project: &ProjectBinding) -> Vec<PathBuf> {
        let mut roots = project
            .root_path
            .as_deref()
            .map(Self::discover_skill_roots)
            .unwrap_or_default();
        if !roots.iter().any(|root| root == &project.skills_dir) {
            roots.insert(0, project.skills_dir.clone());
        } else if let Some(index) = roots.iter().position(|root| root == &project.skills_dir) {
            let primary = roots.remove(index);
            roots.insert(0, primary);
        }
        roots
    }

    /// Resolve the project-local config directory for a managed agent.
    ///
    /// Vercel Skills uses project-local agent directories, while legacy
    /// bindings without a repository root continue to use the global tool
    /// configuration. Custom or legacy tool ids fall back to their configured
    /// global path until an explicit agent definition is provided.
    pub fn project_tool_config_dir(project: &ProjectBinding, tool_id: &str) -> Option<PathBuf> {
        let root = project.root_path.as_ref()?;
        let definition = SUPPORTED_TOOLS
            .iter()
            .find(|definition| definition.id == tool_id)?;
        let primary = root.join(definition.config_dir);

        if primary.exists() {
            return Some(primary);
        }

        definition
            .alt_config_dirs
            .iter()
            .map(|alternate| root.join(alternate))
            .find(|candidate| candidate.exists())
            .or(Some(primary))
    }

    pub fn project_tool_skills_dir(project: &ProjectBinding, tool_id: &str) -> Option<PathBuf> {
        Self::project_tool_config_dir(project, tool_id).map(|path| path.join("skills"))
    }

    pub fn preview_project(
        path: &str,
        project_name: Option<&str>,
    ) -> Result<ProjectBinding, String> {
        Self::build_project_binding(Path::new(path), project_name)
    }

    pub fn register_project(path: &str, project_name: Option<&str>) -> Result<AppConfig, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let binding = Self::build_project_binding(Path::new(path), project_name)?;

        if config.projects.iter().any(|project| {
            project.id == binding.id
                || project.skills_dir == binding.skills_dir
                || (project.root_path.is_some() && project.root_path == binding.root_path)
        }) {
            return Err(format!("Project is already registered: {}", binding.name));
        }

        config.projects.push(binding.clone());
        if config.active_project_id.is_none() {
            config.active_project_id = Some(binding.id);
        }
        manager.save(&config)?;
        Ok(config)
    }

    pub fn set_active_project(project_id: Option<&str>) -> Result<AppConfig, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        if let Some(project_id) = project_id {
            if !config
                .projects
                .iter()
                .any(|project| project.id == project_id)
            {
                return Err(format!("Project not found: {project_id}"));
            }
            config.active_project_id = Some(project_id.to_string());
        } else {
            config.active_project_id = None;
        }
        manager.save(&config)?;
        Ok(config)
    }

    pub fn remove_project(project_id: &str) -> Result<AppConfig, String> {
        let manager = ConfigManager::new();
        let mut config = manager.load()?;
        let original_len = config.projects.len();
        config.projects.retain(|project| project.id != project_id);
        if config.projects.len() == original_len {
            return Err(format!("Project not found: {project_id}"));
        }
        if config.active_project_id.as_deref() == Some(project_id) {
            config.active_project_id = config.projects.first().map(|project| project.id.clone());
        }
        manager.save(&config)?;
        Ok(config)
    }
}

fn normalize_existing_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!(
            "Workspace directory does not exist: {}",
            path.display()
        ));
    }
    let canonical = std::fs::canonicalize(path)
        .map_err(|error| format!("Failed to resolve workspace directory: {error}"))?;

    #[cfg(windows)]
    if let Some(path) = canonical
        .to_str()
        .and_then(|value| value.strip_prefix("\\\\?\\"))
    {
        return Ok(PathBuf::from(path));
    }

    Ok(canonical)
}

fn is_skill_dir(path: &Path) -> bool {
    path.join("SKILL.md").is_file()
        || path.join("skill.md").is_file()
        || path.join("meta.json").is_file()
}

fn default_project_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace");
    if name.eq_ignore_ascii_case("skills") {
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or(name)
            .to_string()
    } else {
        name.to_string()
    }
}

fn project_id(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().to_lowercase().hash(&mut hasher);
    format!("workspace-{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::WorkspaceService;
    use crate::test_support::with_temp_home;
    use std::fs;

    #[test]
    fn discovers_vercel_skills_repository_roots() {
        with_temp_home(|home| {
            let repository = home.join("repo");
            fs::create_dir_all(repository.join("skills").join("alpha")).unwrap();
            fs::create_dir_all(repository.join(".agents").join("skills").join("beta")).unwrap();

            let roots = WorkspaceService::discover_skill_roots(&repository);

            assert_eq!(roots[0], repository.join("skills"));
            assert!(roots.contains(&repository.join(".agents").join("skills")));
        });
    }

    #[test]
    fn registers_repository_root_without_breaking_skills_directory_binding() {
        with_temp_home(|home| {
            let repository = home.join("skills-manager");
            fs::create_dir_all(repository.join("skills").join("alpha")).unwrap();

            let binding = WorkspaceService::build_project_binding(&repository, None).unwrap();

            assert_eq!(binding.name, "skills-manager");
            assert_eq!(binding.skills_dir, repository.join("skills"));
            assert_eq!(binding.root_path, Some(repository));
        });
    }

    #[test]
    fn registers_empty_git_repository_for_future_skill_discovery() {
        with_temp_home(|home| {
            let repository = home.join("empty-repo");
            fs::create_dir_all(repository.join(".git")).unwrap();

            let binding = WorkspaceService::build_project_binding(&repository, None).unwrap();

            assert_eq!(binding.root_path, Some(repository.clone()));
            assert_eq!(binding.skills_dir, repository.join("skills"));
        });
    }

    #[test]
    fn resolves_project_local_agent_targets_from_repository_root() {
        with_temp_home(|home| {
            let repository = home.join("targeted-repo");
            fs::create_dir_all(repository.join("skills")).unwrap();
            let binding = WorkspaceService::build_project_binding(&repository, None).unwrap();

            assert_eq!(
                WorkspaceService::project_tool_skills_dir(&binding, "claude-code"),
                Some(repository.join(".claude").join("skills"))
            );
            assert_eq!(
                WorkspaceService::project_tool_skills_dir(&binding, "vercel-skills"),
                Some(repository.join(".agents").join("skills"))
            );
        });
    }
}
