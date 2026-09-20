use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use sm_core::models::{
    home_dir, AppConfig, ClawhubSkillFilesResponse, InstallResult, InstallStatus,
    MarketplaceInstallation, MarketplaceSkill, MarketplaceSkillsResponse, MarketplaceSource,
    MarketplaceSyncResult, MarketplaceUpdateCheckResult, Skill, SkillFileNode, SkillScope,
    SkillSource,
};
use sm_core::services::marketplace::{
    derive_github_repo_and_skill_path, CLAWHUB_SOURCE_ID, DIRECT_GITHUB_SOURCE_ID,
    DIRECT_GITHUB_SOURCE_NAME,
};
use sm_core::services::{
    project_tool_skills_dir, AppCache, ConfigManager, LinkerService, MarketplaceCache,
    MarketplaceService, ScannerService,
};

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceSkillDescriptionRequest {
    pub id: String,
    pub repo_url: String,
    pub skill_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceSkillReference {
    pub name: String,
    pub marketplace_source_id: Option<String>,
    pub marketplace_skill_id: Option<String>,
    pub marketplace_skill_slug: Option<String>,
    pub repo_url: Option<String>,
    pub skill_path: Option<String>,
    pub remote_revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MarketplaceInstallTarget {
    pub scope: SkillScope,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedProjectToolTarget {
    tool_id: String,
    skills_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedMarketplaceInstallTarget {
    skills_dir: PathBuf,
    scope: SkillScope,
    project_id: Option<String>,
    project_name: Option<String>,
    project_tool_targets: Vec<ResolvedProjectToolTarget>,
    project_tool_cleanup_targets: Vec<ResolvedProjectToolTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMarketplaceUpdateCheckState {
    last_checked_at_unix_secs: u64,
}

const MARKETPLACE_UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

fn resolve_marketplace_install_target(
    config: &AppConfig,
    target: Option<MarketplaceInstallTarget>,
) -> Result<ResolvedMarketplaceInstallTarget, String> {
    let target = target.unwrap_or(MarketplaceInstallTarget {
        scope: SkillScope::Global,
        project_id: None,
        tool_ids: Vec::new(),
    });

    match target.scope {
        SkillScope::Global => {
            if !target.tool_ids.is_empty() {
                return Err("global install target must not include project tool ids".to_string());
            }
            Ok(ResolvedMarketplaceInstallTarget {
                skills_dir: config.skills_dir.clone(),
                scope: SkillScope::Global,
                project_id: None,
                project_name: None,
                project_tool_targets: Vec::new(),
                project_tool_cleanup_targets: Vec::new(),
            })
        }
        SkillScope::Project => {
            let project_id = target
                .project_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "project scope requires project_id".to_string())?;

            let project = config
                .projects
                .iter()
                .find(|project| project.id == project_id)
                .ok_or_else(|| format!("project binding not found: {project_id}"))?;

            project.root_path.as_deref().ok_or_else(|| {
                format!(
                    "project binding '{}' has no project root; remove and add it again",
                    project.name
                )
            })?;
            let mut tool_ids = target
                .tool_ids
                .into_iter()
                .map(|tool_id| tool_id.trim().to_string())
                .filter(|tool_id| !tool_id.is_empty())
                .collect::<Vec<_>>();
            tool_ids.sort();
            tool_ids.dedup();
            if tool_ids.is_empty() {
                return Err("project install target requires at least one tool".to_string());
            }

            let mut project_tool_targets = Vec::with_capacity(tool_ids.len());
            for tool_id in tool_ids {
                let tool = config
                    .get_tool_config(&tool_id)
                    .ok_or_else(|| format!("tool not found: {tool_id}"))?;
                if !tool.enabled {
                    return Err(format!("tool is disabled: {tool_id}"));
                }
                project_tool_targets.push(ResolvedProjectToolTarget {
                    skills_dir: project_tool_skills_dir(project, &tool_id)?,
                    tool_id,
                });
            }
            let selected_paths = project_tool_targets
                .iter()
                .map(|target| target.skills_dir.clone())
                .collect::<HashSet<_>>();
            let mut cleanup_paths = HashSet::new();
            let mut project_tool_cleanup_targets = Vec::new();
            for (tool_id, _) in config.collect_tool_configs() {
                let Ok(skills_dir) = project_tool_skills_dir(project, &tool_id) else {
                    continue;
                };
                if selected_paths.contains(&skills_dir) || !cleanup_paths.insert(skills_dir.clone())
                {
                    continue;
                }
                project_tool_cleanup_targets.push(ResolvedProjectToolTarget {
                    tool_id,
                    skills_dir,
                });
            }

            Ok(ResolvedMarketplaceInstallTarget {
                skills_dir: project.skills_dir.clone(),
                scope: SkillScope::Project,
                project_id: Some(project.id.clone()),
                project_name: Some(project.name.clone()),
                project_tool_targets,
                project_tool_cleanup_targets,
            })
        }
        SkillScope::Tool => Err("marketplace installs do not support tool scope".to_string()),
    }
}

fn target_from_installation(installation: &MarketplaceInstallation) -> MarketplaceInstallTarget {
    MarketplaceInstallTarget {
        scope: installation.scope.clone(),
        project_id: installation.project_id.clone(),
        tool_ids: installation.tool_ids.clone(),
    }
}

fn activate_project_installation(
    result: &InstallResult,
    target: &ResolvedMarketplaceInstallTarget,
) -> Result<(), String> {
    if target.scope != SkillScope::Project {
        return Ok(());
    }
    let installed_path = result
        .installed_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| "project installation did not return an installed path".to_string())?;
    let skill_id = installed_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "installed Skill path has no valid directory name".to_string())?;

    let rollback_activations = |activated: Vec<ResolvedProjectToolTarget>| {
        for completed in activated.into_iter().rev() {
            let _ = LinkerService::disable_skill_for_tool(
                &completed.skills_dir,
                skill_id,
                &completed.tool_id,
            );
        }
    };

    let mut activated: Vec<ResolvedProjectToolTarget> = Vec::new();
    for tool_target in &target.project_tool_targets {
        if installed_path == tool_target.skills_dir.join(skill_id) {
            continue;
        }
        match LinkerService::check_link_for_scoped_skill(
            &installed_path,
            &tool_target.skills_dir,
            skill_id,
            &tool_target.tool_id,
            &SkillScope::Project,
        ) {
            sm_core::services::LinkStatus::Valid => continue,
            sm_core::services::LinkStatus::Missing => {}
            status => {
                rollback_activations(activated);
                return Err(format!(
                    "installed Skill but refused to replace the existing {} target for {}: {:?}",
                    skill_id, tool_target.tool_id, status
                ));
            }
        }
        if let Err(error) = LinkerService::enable_skill_for_tool(
            &installed_path,
            &tool_target.skills_dir,
            skill_id,
            &tool_target.tool_id,
        ) {
            rollback_activations(activated);
            return Err(format!(
                "installed Skill but failed to activate it for {}: {}",
                tool_target.tool_id, error
            ));
        }
        activated.push(tool_target.clone());
    }

    let mut cleaned: Vec<ResolvedProjectToolTarget> = Vec::new();
    for tool_target in &target.project_tool_cleanup_targets {
        if LinkerService::check_link_for_scoped_skill(
            &installed_path,
            &tool_target.skills_dir,
            skill_id,
            &tool_target.tool_id,
            &SkillScope::Project,
        ) != sm_core::services::LinkStatus::Valid
        {
            continue;
        }
        if let Err(error) = LinkerService::disable_skill_for_tool(
            &tool_target.skills_dir,
            skill_id,
            &tool_target.tool_id,
        ) {
            for completed in cleaned.into_iter().rev() {
                let _ = LinkerService::enable_skill_for_tool(
                    &installed_path,
                    &completed.skills_dir,
                    skill_id,
                    &completed.tool_id,
                );
            }
            rollback_activations(activated);
            return Err(format!(
                "installed Skill but failed to deactivate it for {}: {}",
                tool_target.tool_id, error
            ));
        }
        cleaned.push(tool_target.clone());
    }
    Ok(())
}

fn installation_scope_label(installation: &MarketplaceInstallation) -> String {
    match installation.scope {
        SkillScope::Global => "global".to_string(),
        SkillScope::Project => installation
            .project_name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .map(|name| format!("project: {name}"))
            .unwrap_or_else(|| "project".to_string()),
        SkillScope::Tool => "tool".to_string(),
    }
}

fn github_token_from_config(config: &AppConfig) -> Option<String> {
    config
        .preferences
        .as_ref()
        .and_then(|prefs| prefs.github_token.clone())
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn normalize_source_filter(source_ids: Option<Vec<String>>) -> Option<Vec<String>> {
    let mut ids: Vec<String> = source_ids
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

fn build_marketplace_skill_from_reference(
    reference: MarketplaceSkillReference,
) -> Result<MarketplaceSkill, String> {
    let raw_repo_url = reference.repo_url.unwrap_or_default().trim().to_string();
    if raw_repo_url.is_empty() {
        return Err("repo_url is required".to_string());
    }
    let (derived_repo_url, derived_skill_path) =
        derive_github_repo_and_skill_path(Some(raw_repo_url.as_str()), "");
    let repo_url = derived_repo_url.unwrap_or_else(|| raw_repo_url.clone());
    let is_github_reference = repo_url.contains("github.com/");

    let mut skill_path = reference.skill_path.unwrap_or_default().trim().to_string();
    if skill_path.is_empty() {
        if let Some(derived) = derived_skill_path {
            skill_path = derived;
        }
    }
    if skill_path.is_empty() && !is_github_reference {
        return Err("skill_path is required".to_string());
    }
    let source_id = reference.marketplace_source_id.unwrap_or_else(|| {
        if is_github_reference {
            DIRECT_GITHUB_SOURCE_ID.to_string()
        } else {
            "marketplace".to_string()
        }
    });
    let slug = reference.marketplace_skill_slug.clone().or_else(|| {
        if skill_path.is_empty() {
            repo_url
                .rsplit('/')
                .next()
                .map(str::to_string)
                .filter(|value| !value.trim().is_empty())
        } else {
            Some(skill_path.clone())
        }
    });
    let fallback_id = build_reference_skill_id(&source_id, &repo_url, &skill_path, slug.as_deref());
    let name = reference.name.trim().to_string();
    let name = if name.is_empty() {
        skill_display_name(slug.as_deref(), &repo_url, &skill_path)
            .unwrap_or("skill")
            .to_string()
    } else {
        name
    };

    Ok(MarketplaceSkill {
        id: reference
            .marketplace_skill_id
            .clone()
            .unwrap_or(fallback_id),
        slug,
        name,
        description: None,
        author: None,
        source_id: source_id.clone(),
        source_name: source_id,
        install_count: None,
        install_url: None,
        created_at: None,
        repo_url: Some(repo_url),
        skill_path: Some(skill_path),
        external_url: Some(raw_repo_url),
        remote_revision: reference.remote_revision,
        tags: Vec::new(),
        install_status: InstallStatus::NotInstalled,
        installations: Vec::new(),
        clawhub_slug: None,
        clawhub_owner: None,
        clawhub_version: None,
    })
}

fn build_reference_skill_id(
    source_id: &str,
    repo_url: &str,
    skill_path: &str,
    slug: Option<&str>,
) -> String {
    let raw = if source_id == DIRECT_GITHUB_SOURCE_ID {
        format!("{}-{}-{}", source_id, repo_url.trim(), skill_path.trim())
    } else {
        format!("{}-{}", source_id, slug.unwrap_or(skill_path))
    };
    raw.to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn is_remote_skill_manifest_name(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "skill.md" | "readme.md"
    )
}

fn expand_skill_group_reference(
    skill: &MarketplaceSkill,
    tree: &SkillFileNode,
) -> Vec<MarketplaceSkill> {
    let root_has_manifest = tree.children.as_ref().is_some_and(|children| {
        children
            .iter()
            .any(|child| !child.is_dir && is_remote_skill_manifest_name(&child.name))
    });
    if root_has_manifest {
        return Vec::new();
    }

    let Some(children) = tree.children.as_ref() else {
        return Vec::new();
    };

    children
        .iter()
        .filter(|child| child.is_dir)
        .filter(|child| {
            child.children.as_ref().is_some_and(|entries| {
                entries
                    .iter()
                    .any(|entry| !entry.is_dir && is_remote_skill_manifest_name(&entry.name))
            })
        })
        .map(|child| {
            let child_path = child.path.clone();
            MarketplaceSkill {
                id: build_reference_skill_id(
                    &skill.source_id,
                    skill.repo_url.as_deref().unwrap_or_default(),
                    &child_path,
                    Some(child_path.as_str()),
                ),
                slug: Some(child_path.clone()),
                name: child.name.clone(),
                description: None,
                author: None,
                source_id: skill.source_id.clone(),
                source_name: skill.source_name.clone(),
                install_count: None,
                install_url: skill.install_url.clone(),
                created_at: skill.created_at,
                repo_url: skill.repo_url.clone(),
                skill_path: Some(child_path),
                external_url: skill.external_url.clone(),
                remote_revision: None,
                tags: Vec::new(),
                install_status: InstallStatus::NotInstalled,
                installations: Vec::new(),
                clawhub_slug: None,
                clawhub_owner: None,
                clawhub_version: None,
            }
        })
        .collect()
}

fn skill_display_name<'a>(
    slug: Option<&'a str>,
    repo_url: &'a str,
    skill_path: &'a str,
) -> Option<&'a str> {
    if !skill_path.trim().is_empty() {
        return skill_path
            .rsplit('/')
            .next()
            .filter(|value| !value.trim().is_empty());
    }
    slug.and_then(|value| value.rsplit('/').next())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            repo_url
                .rsplit('/')
                .next()
                .filter(|value| !value.trim().is_empty())
        })
}

fn resolve_marketplace_source_name(
    source_id: &str,
    source_name_by_id: &HashMap<String, String>,
) -> String {
    source_name_by_id
        .get(source_id)
        .cloned()
        .unwrap_or_else(|| {
            if source_id == DIRECT_GITHUB_SOURCE_ID {
                DIRECT_GITHUB_SOURCE_NAME.to_string()
            } else {
                source_id.to_string()
            }
        })
}

fn resolve_cache_source_scope(
    normalized_source_filter: &Option<Vec<String>>,
    sources: &[MarketplaceSource],
) -> Option<Vec<String>> {
    let mut enabled_ids: Vec<String> = sources
        .iter()
        .filter(|source| source.enabled)
        .map(|source| source.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    enabled_ids.sort();
    enabled_ids.dedup();

    match normalized_source_filter {
        Some(explicit_ids) => {
            let enabled_set: HashSet<&str> = enabled_ids.iter().map(String::as_str).collect();
            let mut scoped_ids: Vec<String> = explicit_ids
                .iter()
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty() && enabled_set.contains(id.as_str()))
                .collect();
            scoped_ids.sort();
            scoped_ids.dedup();
            Some(scoped_ids)
        }
        None => Some(enabled_ids),
    }
}

fn load_marketplace_skills(config: &AppConfig) -> Result<Vec<Skill>, String> {
    // Project bindings can change while the application is running. Always scan
    // against the freshly loaded config so Marketplace state includes every
    // configured project, independent of the active Skills-page context.
    ScannerService::scan_all_scoped_skills(config)
}

fn local_marketplace_skill_id(skill: &Skill) -> Option<&str> {
    if !matches!(skill.source, SkillSource::Marketplace) {
        return None;
    }
    skill
        .marketplace_meta
        .as_ref()?
        .marketplace_skill_id
        .as_deref()
}

fn installation_status_for_local_skill(
    marketplace_skill: &MarketplaceSkill,
    local_skill: &Skill,
) -> InstallStatus {
    let remote_revision = marketplace_skill
        .remote_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let local_revision = local_skill
        .marketplace_meta
        .as_ref()
        .and_then(|meta| meta.remote_revision.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match (remote_revision, local_revision) {
        (Some(remote), Some(local)) if remote != local => InstallStatus::UpdateAvailable,
        (Some(_), None) => InstallStatus::UpdateAvailable,
        _ => InstallStatus::Installed,
    }
}

fn collect_marketplace_installations(
    marketplace_skill: &MarketplaceSkill,
    local_skills: &[Skill],
) -> Vec<MarketplaceInstallation> {
    let mut installations: Vec<MarketplaceInstallation> = local_skills
        .iter()
        .filter(|local_skill| {
            local_marketplace_skill_id(local_skill) == Some(marketplace_skill.id.as_str())
        })
        .map(|local_skill| MarketplaceInstallation {
            instance_id: local_skill.instance_id.clone(),
            scope: local_skill.scope.clone(),
            project_id: local_skill.project_id.clone(),
            project_name: local_skill.project_name.clone(),
            tool_ids: {
                let mut tool_ids = local_skill
                    .enabled
                    .iter()
                    .filter_map(|(tool_id, enabled)| enabled.then_some(tool_id.clone()))
                    .collect::<Vec<_>>();
                tool_ids.sort();
                tool_ids
            },
            install_status: installation_status_for_local_skill(marketplace_skill, local_skill),
        })
        .collect();

    installations.sort_by(|a, b| {
        let a_scope = if a.scope == SkillScope::Global { 0 } else { 1 };
        let b_scope = if b.scope == SkillScope::Global { 0 } else { 1 };
        a_scope
            .cmp(&b_scope)
            .then_with(|| a.project_name.cmp(&b.project_name))
            .then_with(|| a.instance_id.cmp(&b.instance_id))
    });
    installations
}

fn aggregate_install_status(installations: &[MarketplaceInstallation]) -> InstallStatus {
    if installations
        .iter()
        .any(|item| item.install_status == InstallStatus::UpdateAvailable)
    {
        InstallStatus::UpdateAvailable
    } else if installations
        .iter()
        .any(|item| item.install_status == InstallStatus::Installed)
    {
        InstallStatus::Installed
    } else {
        InstallStatus::NotInstalled
    }
}

fn collect_marketplace_update_candidates(
    skills: &[MarketplaceSkill],
) -> Vec<(MarketplaceSkill, MarketplaceInstallation)> {
    skills
        .iter()
        .flat_map(|skill| {
            skill
                .installations
                .iter()
                .filter(|installation| {
                    installation.install_status == InstallStatus::UpdateAvailable
                })
                .map(|installation| (skill.clone(), installation.clone()))
        })
        .collect()
}

fn with_marketplace_installations(
    mut marketplace_skill: MarketplaceSkill,
    local_skills: &[Skill],
) -> MarketplaceSkill {
    marketplace_skill.installations =
        collect_marketplace_installations(&marketplace_skill, local_skills);
    marketplace_skill.install_status = aggregate_install_status(&marketplace_skill.installations);
    marketplace_skill
}

fn collect_installed_marketplace_skills(
    skills: &[Skill],
    sources: &[MarketplaceSource],
    normalized_query: Option<&str>,
    normalized_source_filter: &Option<Vec<String>>,
) -> Vec<MarketplaceSkill> {
    let source_name_by_id: HashMap<String, String> = sources
        .iter()
        .map(|source| (source.id.clone(), source.name.clone()))
        .collect();
    let selected_source_ids: Option<HashSet<&str>> = normalized_source_filter
        .as_ref()
        .map(|ids| ids.iter().map(String::as_str).collect());

    let mut installed_by_id: HashMap<String, MarketplaceSkill> = HashMap::new();
    for skill in skills
        .iter()
        .filter(|skill| matches!(skill.source, SkillSource::Marketplace))
    {
        let Some(meta) = skill.marketplace_meta.as_ref() else {
            continue;
        };
        let source_id = meta
            .marketplace_source_id
            .clone()
            .unwrap_or_else(|| "marketplace".to_string());

        if let Some(filter) = &selected_source_ids {
            if !filter.contains(source_id.as_str()) {
                continue;
            }
        }

        let Some(marketplace_skill_id) = meta.marketplace_skill_id.clone() else {
            continue;
        };
        installed_by_id
            .entry(marketplace_skill_id.clone())
            .or_insert_with(|| MarketplaceSkill {
                id: marketplace_skill_id,
                slug: meta
                    .marketplace_skill_slug
                    .clone()
                    .or_else(|| meta.skill_path.clone()),
                name: skill.name.clone(),
                description: skill.description.clone(),
                author: None,
                source_id: source_id.clone(),
                source_name: resolve_marketplace_source_name(&source_id, &source_name_by_id),
                install_count: None,
                install_url: None,
                created_at: None,
                repo_url: meta.repo_url.clone(),
                skill_path: meta.skill_path.clone(),
                external_url: meta.repo_url.clone(),
                remote_revision: meta.remote_revision.clone(),
                tags: Vec::new(),
                install_status: InstallStatus::Installed,
                installations: Vec::new(),
                clawhub_slug: if source_id == CLAWHUB_SOURCE_ID {
                    meta.marketplace_skill_slug
                        .clone()
                        .or_else(|| meta.skill_path.clone())
                } else {
                    None
                },
                clawhub_owner: None,
                clawhub_version: None,
            });
    }

    let mut installed: Vec<MarketplaceSkill> = installed_by_id
        .into_values()
        .map(|skill| with_marketplace_installations(skill, skills))
        .collect();

    installed.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    MarketplaceService::filter_marketplace_skills_by_query(installed, normalized_query)
}

fn prepend_missing_installed_marketplace_skills(
    response: MarketplaceSkillsResponse,
    installed_skills: Vec<MarketplaceSkill>,
) -> MarketplaceSkillsResponse {
    let existing_ids: HashSet<&str> = response
        .skills
        .iter()
        .map(|skill| skill.id.as_str())
        .collect();
    let mut merged: Vec<MarketplaceSkill> = installed_skills
        .into_iter()
        .filter(|skill| !existing_ids.contains(skill.id.as_str()))
        .collect();
    merged.extend(response.skills);

    MarketplaceSkillsResponse {
        skills: merged,
        has_more: response.has_more,
    }
}

fn should_hydrate_missing_installed_marketplace_skill(skill: &MarketplaceSkill) -> bool {
    (skill.source_id == DIRECT_GITHUB_SOURCE_ID
        && skill
            .repo_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && skill
            .skill_path
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()))
        || (skill.source_id == CLAWHUB_SOURCE_ID
            && skill
                .clawhub_slug
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()))
}

async fn merge_installed_marketplace_skills_into_page(
    mut response: MarketplaceSkillsResponse,
    page: u32,
    skills: &[Skill],
    sources: &[MarketplaceSource],
    normalized_query: Option<&str>,
    normalized_source_filter: &Option<Vec<String>>,
    github_token: Option<&str>,
) -> MarketplaceSkillsResponse {
    if page != 1 {
        response.skills = response
            .skills
            .into_iter()
            .map(|skill| with_marketplace_installations(skill, skills))
            .collect();
        return response;
    }

    let installed_skills = collect_installed_marketplace_skills(
        skills,
        sources,
        normalized_query,
        normalized_source_filter,
    );

    let existing_ids: HashSet<String> = response
        .skills
        .iter()
        .map(|skill| skill.id.clone())
        .collect();
    let mut hydrated_installed = Vec::new();
    for skill in installed_skills
        .into_iter()
        .filter(|skill| !existing_ids.contains(&skill.id))
    {
        let resolved = if should_hydrate_missing_installed_marketplace_skill(&skill) {
            match MarketplaceService::hydrate_marketplace_skill(&skill, github_token).await {
                Ok(resolved) => resolved,
                Err(_) => skill,
            }
        } else {
            skill
        };
        hydrated_installed.push(resolved);
    }

    let mut merged = prepend_missing_installed_marketplace_skills(response, hydrated_installed);
    merged.skills = merged
        .skills
        .into_iter()
        .map(|skill| with_marketplace_installations(skill, skills))
        .collect();
    merged
}

fn marketplace_update_check_state_path() -> Option<PathBuf> {
    Some(
        home_dir()?
            .join(".skills-manager")
            .join("cache")
            .join("marketplace-update-check.json"),
    )
}

fn load_last_update_check_time() -> Option<SystemTime> {
    let path = marketplace_update_check_state_path()?;
    let content = fs::read_to_string(path).ok()?;
    let state: PersistedMarketplaceUpdateCheckState = serde_json::from_str(&content).ok()?;
    Some(UNIX_EPOCH + Duration::from_secs(state.last_checked_at_unix_secs))
}

fn persist_update_check_time(checked_at: SystemTime) {
    let Some(path) = marketplace_update_check_state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    let state = PersistedMarketplaceUpdateCheckState {
        last_checked_at_unix_secs: checked_at
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs())
            .unwrap_or_default(),
    };

    if let Ok(content) = serde_json::to_string(&state) {
        let _ = fs::write(path, content);
    }
}

fn should_run_marketplace_update_check(last_checked: Option<SystemTime>, now: SystemTime) -> bool {
    match last_checked {
        None => true,
        Some(last) => now
            .duration_since(last)
            .map(|elapsed| elapsed >= MARKETPLACE_UPDATE_CHECK_INTERVAL)
            .unwrap_or(true),
    }
}

#[tauri::command]
pub async fn fetch_marketplace_skills(
    force_refresh: bool,
    query: Option<String>,
    page: Option<u32>,
    source_ids: Option<Vec<String>>,
    cache: State<'_, MarketplaceCache>,
) -> Result<MarketplaceSkillsResponse, String> {
    let normalized_query = query
        .as_ref()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let normalized_source_filter = normalize_source_filter(source_ids);
    let page = page.unwrap_or(1).max(1);
    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    let cache_source_scope = resolve_cache_source_scope(
        &normalized_source_filter,
        config.marketplace_sources.as_deref().unwrap_or(&[]),
    );
    let sources = config.marketplace_sources.clone().unwrap_or_default();

    if !force_refresh {
        if let Some(cached) =
            cache.get_fresh_with_meta(page, &normalized_query, &cache_source_scope)
        {
            let installed_skills = load_marketplace_skills(&config)?;
            return Ok(merge_installed_marketplace_skills_into_page(
                cached,
                page,
                &installed_skills,
                &sources,
                normalized_query.as_deref(),
                &cache_source_scope,
                github_token.as_deref(),
            )
            .await);
        }
    }

    let runtime_cache_source_scope =
        resolve_cache_source_scope(&normalized_source_filter, &sources);

    let result = match MarketplaceService::fetch_marketplace_skills_page(
        &config.skills_dir,
        normalized_query.clone(),
        page,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            if page == 1 {
                if let Some(cached) = cache.get_any() {
                    let runtime_scope_ids = runtime_cache_source_scope.clone().unwrap_or_default();
                    let runtime_scope_set: HashSet<&str> =
                        runtime_scope_ids.iter().map(String::as_str).collect();
                    let filtered_by_source: Vec<MarketplaceSkill> = if !runtime_scope_set.is_empty()
                        || runtime_cache_source_scope
                            .as_ref()
                            .is_some_and(|ids| ids.is_empty())
                    {
                        cached
                            .into_iter()
                            .filter(|skill| runtime_scope_set.contains(skill.source_id.as_str()))
                            .collect()
                    } else {
                        cached
                    };
                    let filtered = MarketplaceService::filter_marketplace_skills_by_query(
                        filtered_by_source,
                        normalized_query.as_deref(),
                    );
                    let installed_skills = load_marketplace_skills(&config)?;
                    return Ok(merge_installed_marketplace_skills_into_page(
                        MarketplaceSkillsResponse {
                            skills: filtered,
                            has_more: false,
                        },
                        page,
                        &installed_skills,
                        &sources,
                        normalized_query.as_deref(),
                        &runtime_cache_source_scope,
                        github_token.as_deref(),
                    )
                    .await);
                }
            }
            return Err(err);
        }
    };

    cache.set_page(
        page,
        normalized_query.clone(),
        runtime_cache_source_scope.clone(),
        result.clone(),
    );

    let installed_skills = load_marketplace_skills(&config)?;
    Ok(merge_installed_marketplace_skills_into_page(
        result,
        page,
        &installed_skills,
        &sources,
        normalized_query.as_deref(),
        &runtime_cache_source_scope,
        github_token.as_deref(),
    )
    .await)
}

#[tauri::command]
pub async fn fetch_skill_files(
    repo_url: String,
    skill_path: String,
) -> Result<SkillFileNode, String> {
    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    MarketplaceService::fetch_skill_files(&repo_url, &skill_path, github_token.as_deref()).await
}

#[tauri::command]
pub async fn fetch_clawhub_skill_files(
    slug: String,
    owner: Option<String>,
    version: Option<String>,
) -> Result<ClawhubSkillFilesResponse, String> {
    MarketplaceService::fetch_clawhub_skill_files(&slug, owner.as_deref(), version.as_deref()).await
}

#[tauri::command]
pub async fn fetch_skill_file_content(download_url: String) -> Result<String, String> {
    MarketplaceService::fetch_skill_file_content(&download_url).await
}

#[tauri::command]
pub async fn fetch_marketplace_skill_descriptions(
    skills: Vec<MarketplaceSkillDescriptionRequest>,
) -> Result<HashMap<String, Option<String>>, String> {
    if skills.is_empty() {
        return Ok(HashMap::new());
    }

    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);

    let mut descriptions = HashMap::with_capacity(skills.len());
    for skill in skills {
        if skill.repo_url.trim().is_empty() || skill.skill_path.trim().is_empty() {
            descriptions.insert(skill.id, None);
            continue;
        }
        let description = MarketplaceService::fetch_skill_description(
            &skill.repo_url,
            &skill.skill_path,
            github_token.as_deref(),
        )
        .await;
        descriptions.insert(skill.id, description);
    }

    Ok(descriptions)
}

#[tauri::command]
pub async fn install_marketplace_skill(
    skill_id: String,
    target: Option<MarketplaceInstallTarget>,
    marketplace_cache: State<'_, MarketplaceCache>,
    app_cache: State<'_, AppCache>,
) -> Result<InstallResult, String> {
    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    let resolved_target = resolve_marketplace_install_target(&config, target)?;

    let skill = if let Some(skill) = marketplace_cache.get_cached_skill(&skill_id) {
        skill
    } else {
        return Err("未找到对应的 Skill，请先在市场列表中加载该技能后再安装".to_string());
    };

    let result = MarketplaceService::install_skill(
        &skill,
        &resolved_target.skills_dir,
        github_token.as_deref(),
    )
    .await?;
    let activation_result = activate_project_installation(&result, &resolved_target);

    // Local installation state is recomputed on every Marketplace read. Keep the
    // remote listing cached so a multi-target frontend request can reuse the same
    // Marketplace Skill for subsequent project installations.
    app_cache.invalidate_skills();
    activation_result?;

    Ok(result)
}

#[tauri::command]
pub async fn install_marketplace_skill_by_ref(
    reference: MarketplaceSkillReference,
    target: Option<MarketplaceInstallTarget>,
    marketplace_cache: State<'_, MarketplaceCache>,
    app_cache: State<'_, AppCache>,
) -> Result<InstallResult, String> {
    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    let resolved_target = resolve_marketplace_install_target(&config, target)?;

    let skill = build_marketplace_skill_from_reference(reference)?;
    let result = if let Some(repo_url) = skill.repo_url.as_deref() {
        let requested_path = skill.skill_path.as_deref().unwrap_or_default();
        let tree = MarketplaceService::fetch_skill_files(
            repo_url,
            requested_path,
            github_token.as_deref(),
        )
        .await?;
        let group_members = expand_skill_group_reference(&skill, &tree);

        if group_members.is_empty() {
            let result = MarketplaceService::install_skill(
                &skill,
                &resolved_target.skills_dir,
                github_token.as_deref(),
            )
            .await?;
            if let Err(error) = activate_project_installation(&result, &resolved_target) {
                app_cache.invalidate_skills();
                marketplace_cache.invalidate();
                return Err(error);
            }
            result
        } else {
            for member in &group_members {
                let member_result = MarketplaceService::install_skill(
                    member,
                    &resolved_target.skills_dir,
                    github_token.as_deref(),
                )
                .await?;
                if let Err(error) = activate_project_installation(&member_result, &resolved_target)
                {
                    app_cache.invalidate_skills();
                    marketplace_cache.invalidate();
                    return Err(error);
                }
            }
            InstallResult {
                success: true,
                skill_id: skill.id.clone(),
                message: Some(format!("已安装 {} 个 Skills", group_members.len())),
                installed_path: Some(resolved_target.skills_dir.to_string_lossy().into_owned()),
            }
        }
    } else {
        let result = MarketplaceService::install_skill(
            &skill,
            &resolved_target.skills_dir,
            github_token.as_deref(),
        )
        .await?;
        if let Err(error) = activate_project_installation(&result, &resolved_target) {
            app_cache.invalidate_skills();
            marketplace_cache.invalidate();
            return Err(error);
        }
        result
    };

    app_cache.invalidate_skills();
    marketplace_cache.invalidate();

    Ok(result)
}

#[tauri::command]
pub async fn sync_marketplace_installed_skills(
    source_ids: Option<Vec<String>>,
    marketplace_cache: State<'_, MarketplaceCache>,
    app_cache: State<'_, AppCache>,
) -> Result<MarketplaceSyncResult, String> {
    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    let normalized_source_filter = normalize_source_filter(source_ids);
    let sources = config.marketplace_sources.clone().unwrap_or_default();

    let mut listing =
        MarketplaceService::fetch_marketplace_skills_page(&config.skills_dir, None, 1).await?;
    if let Some(cached_skills) = marketplace_cache.get_any() {
        listing = prepend_missing_installed_marketplace_skills(listing, cached_skills);
    }
    let installed_skills = load_marketplace_skills(&config)?;
    let listing = merge_installed_marketplace_skills_into_page(
        listing,
        1,
        &installed_skills,
        &sources,
        None,
        &normalized_source_filter,
        github_token.as_deref(),
    )
    .await;

    let mut result = MarketplaceSyncResult {
        checked: 0,
        updated: 0,
        failed: Vec::new(),
    };
    let mut installed_any = false;

    for (skill, installation) in collect_marketplace_update_candidates(&listing.skills) {
        result.checked += 1;
        let target = match resolve_marketplace_install_target(
            &config,
            Some(target_from_installation(&installation)),
        ) {
            Ok(target) => target,
            Err(err) => {
                result.failed.push(format!(
                    "{} ({}): {}",
                    skill.name,
                    installation_scope_label(&installation),
                    err
                ));
                continue;
            }
        };

        match MarketplaceService::install_skill(&skill, &target.skills_dir, github_token.as_deref())
            .await
        {
            Ok(install_result) => {
                installed_any = true;
                match activate_project_installation(&install_result, &target) {
                    Ok(()) => result.updated += 1,
                    Err(err) => result.failed.push(format!(
                        "{} ({}): {}",
                        skill.name,
                        installation_scope_label(&installation),
                        err
                    )),
                }
            }
            Err(err) => result.failed.push(format!(
                "{} ({}): {}",
                skill.name,
                installation_scope_label(&installation),
                err
            )),
        }
    }

    if installed_any {
        app_cache.invalidate_skills();
        marketplace_cache.invalidate();
    }

    Ok(result)
}

#[tauri::command]
pub async fn check_marketplace_updates_if_stale(
    marketplace_cache: State<'_, MarketplaceCache>,
) -> Result<MarketplaceUpdateCheckResult, String> {
    let now = SystemTime::now();
    let last_checked = load_last_update_check_time();
    if !should_run_marketplace_update_check(last_checked, now) {
        return Ok(MarketplaceUpdateCheckResult {
            performed: false,
            checked: 0,
            update_available: 0,
        });
    }

    let manager = ConfigManager::new();
    let config = manager.load()?;
    let github_token = github_token_from_config(&config);
    let sources = config.marketplace_sources.clone().unwrap_or_default();

    let listing =
        MarketplaceService::fetch_marketplace_skills_page(&config.skills_dir, None, 1).await?;
    let installed_skills = load_marketplace_skills(&config)?;
    let merged_listing = merge_installed_marketplace_skills_into_page(
        listing.clone(),
        1,
        &installed_skills,
        &sources,
        None,
        &None,
        github_token.as_deref(),
    )
    .await;

    let cache_source_scope = resolve_cache_source_scope(&None, &sources);
    marketplace_cache.set(
        listing.skills.clone(),
        None,
        listing.has_more,
        cache_source_scope,
    );
    persist_update_check_time(now);

    let checked = merged_listing
        .skills
        .iter()
        .flat_map(|skill| skill.installations.iter())
        .count();
    let update_available = merged_listing
        .skills
        .iter()
        .flat_map(|skill| skill.installations.iter())
        .filter(|installation| installation.install_status == InstallStatus::UpdateAvailable)
        .count();

    Ok(MarketplaceUpdateCheckResult {
        performed: true,
        checked,
        update_available,
    })
}

#[tauri::command]
pub async fn get_marketplace_sources() -> Result<Vec<MarketplaceSource>, String> {
    let manager = ConfigManager::new();
    let config = manager.load()?;
    Ok(config.marketplace_sources.clone().unwrap_or_default())
}

#[tauri::command]
pub fn toggle_marketplace_source(source_id: String, enabled: bool) -> Result<(), String> {
    let manager = ConfigManager::new();
    let mut config = manager.load()?;

    let sources = config
        .marketplace_sources
        .get_or_insert_with(|| AppConfig::default().marketplace_sources.unwrap_or_default());

    let mut found = false;
    for source in sources.iter_mut() {
        if source.id == source_id {
            source.enabled = enabled;
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("未找到市场源: {}", source_id));
    }

    manager.save(&config)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use sm_core::models::{
        AppConfig, InstallResult, InstallStatus, MarketplaceMeta, MarketplaceSkill,
        MarketplaceSkillsResponse, MarketplaceSource, ProjectBinding, Skill, SkillFileNode,
        SkillScope, SkillSource, SourceType, ToolConfig,
    };
    use sm_core::services::marketplace::{
        CLAWHUB_SOURCE_ID, DIRECT_GITHUB_SOURCE_ID, DIRECT_GITHUB_SOURCE_NAME,
    };
    use sm_core::test_support::with_temp_home;

    use super::{
        activate_project_installation, aggregate_install_status,
        build_marketplace_skill_from_reference, collect_installed_marketplace_skills,
        collect_marketplace_installations, collect_marketplace_update_candidates,
        expand_skill_group_reference, load_last_update_check_time, persist_update_check_time,
        prepend_missing_installed_marketplace_skills, resolve_cache_source_scope,
        resolve_marketplace_install_target, should_hydrate_missing_installed_marketplace_skill,
        should_run_marketplace_update_check, MarketplaceInstallTarget, MarketplaceSkillReference,
        ResolvedMarketplaceInstallTarget, ResolvedProjectToolTarget,
        MARKETPLACE_UPDATE_CHECK_INTERVAL,
    };

    fn make_source(id: &str, enabled: bool) -> MarketplaceSource {
        MarketplaceSource {
            id: id.to_string(),
            name: id.to_string(),
            url: format!("https://{id}.example.com"),
            source_type: SourceType::Api,
            enabled,
            builtin: true,
            api_key: None,
        }
    }

    fn make_marketplace_skill(
        id: &str,
        source_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Skill {
        Skill {
            id: format!("local-{id}"),
            instance_id: Skill::global_instance_id(&format!("local-{id}")),
            scope: sm_core::models::SkillScope::Global,
            project_id: None,
            project_name: None,
            tool_id: None,
            name: name.to_string(),
            description: description.map(str::to_string),
            version: "1.0.0".to_string(),
            source: SkillSource::Marketplace,
            marketplace_meta: Some(MarketplaceMeta {
                marketplace_source_id: Some(source_id.to_string()),
                marketplace_skill_id: Some(id.to_string()),
                marketplace_skill_slug: Some(name.to_lowercase()),
                repo_url: Some("https://github.com/example/repo".to_string()),
                skill_path: Some(format!(".claude/skills/{}", name.to_lowercase())),
                remote_revision: Some("rev-local".to_string()),
            }),
            vault_meta: None,
            package_meta: None,
            contract: crate::models::SkillContractSummary::unmanaged(),
            enabled: HashMap::new(),
            path: PathBuf::from(format!("/tmp/{id}")),
        }
    }

    fn make_listing_skill(id: &str, install_status: InstallStatus) -> MarketplaceSkill {
        MarketplaceSkill {
            id: id.to_string(),
            slug: Some(id.to_string()),
            name: id.to_string(),
            description: None,
            author: None,
            source_id: "src_skills".to_string(),
            source_name: "src_skills".to_string(),
            install_count: None,
            install_url: None,
            created_at: None,
            repo_url: Some("https://github.com/example/repo".to_string()),
            skill_path: Some(format!(".claude/skills/{id}")),
            external_url: None,
            remote_revision: Some("rev-remote".to_string()),
            tags: Vec::new(),
            install_status,
            installations: Vec::new(),
            clawhub_slug: None,
            clawhub_owner: None,
            clawhub_version: None,
        }
    }

    fn config_with_active_project() -> AppConfig {
        let mut tools = HashMap::new();
        tools.insert(
            "claude-code".to_string(),
            ToolConfig {
                enabled: true,
                detected: true,
                skills_path: PathBuf::from("/configured/global/.claude/skills"),
                config_path: PathBuf::from("/configured/global/.claude"),
            },
        );
        AppConfig {
            skills_dir: PathBuf::from("/configured/global/skills"),
            projects: vec![ProjectBinding {
                id: "project-alpha".to_string(),
                name: "Alpha".to_string(),
                root_path: Some(PathBuf::from("/configured/alpha")),
                skills_dir: PathBuf::from("/configured/alpha/.skills-manager/skills"),
            }],
            tools,
            active_project_id: Some("project-alpha".to_string()),
            ..AppConfig::default()
        }
    }

    #[test]
    fn install_target_defaults_to_global_directory() {
        let config = config_with_active_project();
        let resolved = resolve_marketplace_install_target(&config, None).expect("resolve target");

        assert_eq!(resolved.scope, SkillScope::Global);
        assert_eq!(resolved.skills_dir, config.skills_dir);
        assert_eq!(resolved.project_id, None);
    }

    #[test]
    fn install_target_resolves_explicit_global_directory() {
        let config = config_with_active_project();
        let resolved = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Global,
                project_id: Some("ignored".to_string()),
                tool_ids: Vec::new(),
            }),
        )
        .expect("resolve target");

        assert_eq!(
            resolved.skills_dir,
            PathBuf::from("/configured/global/skills")
        );
        assert_eq!(resolved.project_id, None);
    }

    #[test]
    fn install_target_resolves_configured_project() {
        let config = config_with_active_project();
        let resolved = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-alpha".to_string()),
                tool_ids: vec!["claude-code".to_string()],
            }),
        )
        .expect("resolve target");

        assert_eq!(resolved.scope, SkillScope::Project);
        assert_eq!(
            resolved.skills_dir,
            PathBuf::from("/configured/alpha/.skills-manager/skills")
        );
        assert_eq!(resolved.project_tool_targets.len(), 1);
        assert_eq!(
            resolved.project_tool_targets[0].skills_dir,
            PathBuf::from("/configured/alpha/.claude/skills")
        );
        assert_eq!(resolved.project_name.as_deref(), Some("Alpha"));
    }

    #[test]
    fn install_target_resolves_non_active_configured_project() {
        let mut config = config_with_active_project();
        config.projects.push(ProjectBinding {
            id: "project-beta".to_string(),
            name: "Beta".to_string(),
            root_path: Some(PathBuf::from("/configured/beta")),
            skills_dir: PathBuf::from("/configured/beta/.skills-manager/skills"),
        });

        let resolved = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-beta".to_string()),
                tool_ids: vec!["claude-code".to_string()],
            }),
        )
        .expect("resolve non-active project target");

        assert_eq!(resolved.project_id.as_deref(), Some("project-beta"));
        assert_eq!(resolved.project_name.as_deref(), Some("Beta"));
        assert_eq!(
            resolved.skills_dir,
            PathBuf::from("/configured/beta/.skills-manager/skills")
        );
    }

    #[test]
    fn install_target_rejects_missing_or_unknown_project() {
        let config = config_with_active_project();
        let missing = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: None,
                tool_ids: vec!["claude-code".to_string()],
            }),
        )
        .expect_err("missing project id should fail");
        let unknown = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-other".to_string()),
                tool_ids: vec!["claude-code".to_string()],
            }),
        )
        .expect_err("unknown project id should fail");

        assert!(missing.contains("project_id"));
        assert!(unknown.contains("project binding not found"));
    }

    #[test]
    fn install_target_requires_enabled_supported_project_tools() {
        let mut config = config_with_active_project();
        let no_tools = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-alpha".to_string()),
                tool_ids: Vec::new(),
            }),
        )
        .expect_err("project targets need a tool");
        let unknown = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-alpha".to_string()),
                tool_ids: vec!["unknown-tool".to_string()],
            }),
        )
        .expect_err("unknown tools must fail");
        config.tools.get_mut("claude-code").unwrap().enabled = false;
        let disabled = resolve_marketplace_install_target(
            &config,
            Some(MarketplaceInstallTarget {
                scope: SkillScope::Project,
                project_id: Some("project-alpha".to_string()),
                tool_ids: vec!["claude-code".to_string()],
            }),
        )
        .expect_err("disabled tools must fail");

        assert!(no_tools.contains("at least one tool"));
        assert!(unknown.contains("tool not found"));
        assert!(disabled.contains("tool is disabled"));
    }

    #[cfg(unix)]
    #[test]
    fn project_activation_syncs_selected_paths_and_preserves_unmanaged_conflicts() {
        with_temp_home(|home| {
            let installed_path = home
                .join("project")
                .join(".skills-manager")
                .join("skills")
                .join("demo");
            let selected_dir = home.join("project").join(".claude").join("skills");
            let cleanup_dir = home.join("project").join(".gemini").join("skills");
            let unmanaged_dir = home.join("project").join(".cursor").join("skills");
            fs::create_dir_all(&installed_path).expect("create installed Skill");
            fs::write(installed_path.join("SKILL.md"), "# Demo\n").expect("write Skill");
            fs::create_dir_all(&cleanup_dir).expect("create cleanup directory");
            fs::create_dir_all(unmanaged_dir.join("demo")).expect("create unmanaged conflict");
            std::os::unix::fs::symlink(&installed_path, cleanup_dir.join("demo"))
                .expect("create old managed link");

            let target = ResolvedMarketplaceInstallTarget {
                skills_dir: installed_path.parent().unwrap().to_path_buf(),
                scope: SkillScope::Project,
                project_id: Some("alpha".to_string()),
                project_name: Some("Alpha".to_string()),
                project_tool_targets: vec![ResolvedProjectToolTarget {
                    tool_id: "claude-code".to_string(),
                    skills_dir: selected_dir.clone(),
                }],
                project_tool_cleanup_targets: vec![
                    ResolvedProjectToolTarget {
                        tool_id: "gemini".to_string(),
                        skills_dir: cleanup_dir.clone(),
                    },
                    ResolvedProjectToolTarget {
                        tool_id: "cursor".to_string(),
                        skills_dir: unmanaged_dir.clone(),
                    },
                ],
            };
            let result = InstallResult {
                success: true,
                skill_id: "demo".to_string(),
                message: None,
                installed_path: Some(installed_path.to_string_lossy().into_owned()),
            };

            activate_project_installation(&result, &target).expect("activate project Skill");

            assert_eq!(
                fs::read_link(selected_dir.join("demo")).expect("read selected link"),
                installed_path
            );
            assert!(cleanup_dir.join("demo").symlink_metadata().is_err());
            assert!(unmanaged_dir.join("demo").is_dir());
        });
    }

    #[test]
    fn install_target_does_not_accept_a_frontend_directory() {
        let error = serde_json::from_value::<MarketplaceInstallTarget>(serde_json::json!({
            "scope": "project",
            "project_id": "project-alpha",
            "skills_dir": "/tmp/untrusted"
        }))
        .expect_err("directory fields must be rejected");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn marketplace_installations_preserve_global_and_project_instances() {
        let mut global = make_marketplace_skill("src_skills::alpha", "src_skills", "Alpha", None);
        global.marketplace_meta.as_mut().unwrap().remote_revision = Some("rev-current".to_string());
        let mut project = global.clone().with_scope(
            SkillScope::Project,
            Some("project-alpha".to_string()),
            Some("Alpha Project".to_string()),
        );
        project.path = PathBuf::from("/configured/alpha/.claude/skills/alpha");
        project.marketplace_meta.as_mut().unwrap().remote_revision = Some("rev-old".to_string());
        let mut remote = make_listing_skill("src_skills::alpha", InstallStatus::NotInstalled);
        remote.remote_revision = Some("rev-current".to_string());

        let installations = collect_marketplace_installations(&remote, &[global, project]);

        assert_eq!(installations.len(), 2);
        assert_eq!(installations[0].scope, SkillScope::Global);
        assert_eq!(installations[0].install_status, InstallStatus::Installed);
        assert_eq!(installations[1].scope, SkillScope::Project);
        assert_eq!(
            installations[1].install_status,
            InstallStatus::UpdateAvailable
        );
        assert_eq!(
            aggregate_install_status(&installations),
            InstallStatus::UpdateAvailable
        );
    }

    #[test]
    fn marketplace_installations_support_single_scope_and_ignore_same_name_local_skill() {
        let mut project = make_marketplace_skill("src_skills::alpha", "src_skills", "Alpha", None)
            .with_scope(
                SkillScope::Project,
                Some("project-alpha".to_string()),
                Some("Alpha Project".to_string()),
            );
        project.marketplace_meta.as_mut().unwrap().remote_revision = Some("rev-remote".to_string());
        let local_same_name = Skill::new(
            "alpha-local".to_string(),
            "Alpha".to_string(),
            PathBuf::from("/tmp/alpha-local"),
        );
        let remote = make_listing_skill("src_skills::alpha", InstallStatus::NotInstalled);

        let installations = collect_marketplace_installations(&remote, &[project, local_same_name]);

        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].scope, SkillScope::Project);
        assert_eq!(
            installations[0].project_id.as_deref(),
            Some("project-alpha")
        );
        assert_eq!(
            aggregate_install_status(&installations),
            InstallStatus::Installed
        );
    }

    #[test]
    fn batch_update_candidates_keep_each_instance_directory() {
        let config = config_with_active_project();
        let mut remote = make_listing_skill("src_skills::alpha", InstallStatus::UpdateAvailable);
        remote.installations = vec![
            sm_core::models::MarketplaceInstallation {
                instance_id: "global:alpha".to_string(),
                scope: SkillScope::Global,
                project_id: None,
                project_name: None,
                tool_ids: Vec::new(),
                install_status: InstallStatus::UpdateAvailable,
            },
            sm_core::models::MarketplaceInstallation {
                instance_id: "project:project-alpha:alpha".to_string(),
                scope: SkillScope::Project,
                project_id: Some("project-alpha".to_string()),
                project_name: Some("Alpha".to_string()),
                tool_ids: vec!["claude-code".to_string()],
                install_status: InstallStatus::UpdateAvailable,
            },
        ];

        let candidates = collect_marketplace_update_candidates(&[remote]);
        let resolved_directories = candidates
            .iter()
            .map(|(_, installation)| {
                resolve_marketplace_install_target(
                    &config,
                    Some(super::target_from_installation(installation)),
                )
                .expect("candidate target should resolve")
                .skills_dir
            })
            .collect::<Vec<_>>();

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            resolved_directories,
            vec![
                PathBuf::from("/configured/global/skills"),
                PathBuf::from("/configured/alpha/.skills-manager/skills"),
            ]
        );
    }

    #[test]
    fn should_run_marketplace_update_check_respects_interval() {
        let now = SystemTime::now();
        let just_checked = now
            .checked_sub(Duration::from_secs(60))
            .expect("time should be valid");
        let stale_checked = now
            .checked_sub(MARKETPLACE_UPDATE_CHECK_INTERVAL + Duration::from_secs(1))
            .expect("time should be valid");

        assert!(
            !should_run_marketplace_update_check(Some(just_checked), now),
            "recent check should be skipped"
        );
        assert!(
            should_run_marketplace_update_check(Some(stale_checked), now),
            "stale check should run"
        );
        assert!(
            should_run_marketplace_update_check(None, now),
            "missing check timestamp should run"
        );
    }

    #[test]
    fn update_check_time_round_trip_persists() {
        with_temp_home(|_| {
            let now = SystemTime::now();
            persist_update_check_time(now);
            let loaded = load_last_update_check_time();
            assert!(loaded.is_some(), "expected persisted timestamp");
        });
    }

    #[test]
    fn resolve_cache_source_scope_defaults_to_enabled_sources() {
        let sources = vec![
            make_source("src_skills", true),
            make_source("src_awesome", false),
        ];

        let scope = resolve_cache_source_scope(&None, &sources);

        assert_eq!(
            scope,
            Some(vec!["src_skills".to_string()]),
            "no explicit filter should cache by enabled sources"
        );
    }

    #[test]
    fn resolve_cache_source_scope_intersects_with_enabled_sources() {
        let sources = vec![
            make_source("src_skills", true),
            make_source("src_awesome", false),
        ];
        let explicit = Some(vec![
            "src_awesome".to_string(),
            "src_skills".to_string(),
            "src_skills".to_string(),
        ]);

        let scope = resolve_cache_source_scope(&explicit, &sources);

        assert_eq!(
            scope,
            Some(vec!["src_skills".to_string()]),
            "explicit filter should drop disabled source ids and deduplicate"
        );
    }

    #[test]
    fn build_marketplace_skill_from_reference_requires_repo_url() {
        let reference = MarketplaceSkillReference {
            name: "S1".to_string(),
            marketplace_source_id: Some("source-1".to_string()),
            marketplace_skill_id: Some("source-1::s1".to_string()),
            marketplace_skill_slug: Some("s1".to_string()),
            repo_url: None,
            skill_path: Some(".claude/skills/s1".to_string()),
            remote_revision: None,
        };

        let err = build_marketplace_skill_from_reference(reference).unwrap_err();
        assert!(err.contains("repo_url"));
    }

    #[test]
    fn build_marketplace_skill_from_reference_distinguishes_github_direct_skills_by_repo() {
        let first = build_marketplace_skill_from_reference(MarketplaceSkillReference {
            name: "Demo".to_string(),
            marketplace_source_id: Some("github_direct".to_string()),
            marketplace_skill_id: None,
            marketplace_skill_slug: None,
            repo_url: Some("https://github.com/acme/skills-one".to_string()),
            skill_path: Some("skills/demo".to_string()),
            remote_revision: None,
        })
        .expect("first skill should build");

        let second = build_marketplace_skill_from_reference(MarketplaceSkillReference {
            name: "Demo".to_string(),
            marketplace_source_id: Some("github_direct".to_string()),
            marketplace_skill_id: None,
            marketplace_skill_slug: None,
            repo_url: Some("https://github.com/acme/skills-two".to_string()),
            skill_path: Some("skills/demo".to_string()),
            remote_revision: None,
        })
        .expect("second skill should build");

        assert_ne!(
            first.id, second.id,
            "direct GitHub installs must stay distinct even when skill_path matches"
        );
    }

    #[test]
    fn expand_skill_group_reference_returns_direct_child_skills_when_root_is_container() {
        let skill = MarketplaceSkill {
            id: "github-direct-baoyu-skills".to_string(),
            slug: Some("skills".to_string()),
            name: "skills".to_string(),
            description: None,
            author: None,
            source_id: DIRECT_GITHUB_SOURCE_ID.to_string(),
            source_name: DIRECT_GITHUB_SOURCE_NAME.to_string(),
            install_count: None,
            install_url: None,
            created_at: None,
            repo_url: Some("https://github.com/JimLiu/baoyu-skills".to_string()),
            skill_path: Some("skills".to_string()),
            external_url: Some(
                "https://github.com/JimLiu/baoyu-skills/tree/main/skills".to_string(),
            ),
            remote_revision: None,
            tags: Vec::new(),
            install_status: InstallStatus::NotInstalled,
            installations: Vec::new(),
            clawhub_slug: None,
            clawhub_owner: None,
            clawhub_version: None,
        };
        let tree = SkillFileNode {
            name: "skills".to_string(),
            path: "skills".to_string(),
            is_dir: true,
            download_url: None,
            sha: None,
            children: Some(vec![
                SkillFileNode {
                    name: "baoyu-translate".to_string(),
                    path: "skills/baoyu-translate".to_string(),
                    is_dir: true,
                    download_url: None,
                    sha: None,
                    children: Some(vec![SkillFileNode {
                        name: "SKILL.md".to_string(),
                        path: "skills/baoyu-translate/SKILL.md".to_string(),
                        is_dir: false,
                        download_url: Some("https://example.com/translate".to_string()),
                        sha: None,
                        children: None,
                    }]),
                },
                SkillFileNode {
                    name: "baoyu-slide-deck".to_string(),
                    path: "skills/baoyu-slide-deck".to_string(),
                    is_dir: true,
                    download_url: None,
                    sha: None,
                    children: Some(vec![SkillFileNode {
                        name: "SKILL.md".to_string(),
                        path: "skills/baoyu-slide-deck/SKILL.md".to_string(),
                        is_dir: false,
                        download_url: Some("https://example.com/slides".to_string()),
                        sha: None,
                        children: None,
                    }]),
                },
            ]),
        };

        let expanded = expand_skill_group_reference(&skill, &tree);

        assert_eq!(expanded.len(), 2);
        assert_eq!(
            expanded
                .iter()
                .map(|item| item.skill_path.as_deref().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec!["skills/baoyu-translate", "skills/baoyu-slide-deck"]
        );
        assert_eq!(
            expanded
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["baoyu-translate", "baoyu-slide-deck"]
        );
    }

    #[test]
    fn expand_skill_group_reference_returns_empty_for_regular_skill_root() {
        let skill = MarketplaceSkill {
            id: "github-direct-demo".to_string(),
            slug: Some("skills/demo".to_string()),
            name: "demo".to_string(),
            description: None,
            author: None,
            source_id: DIRECT_GITHUB_SOURCE_ID.to_string(),
            source_name: DIRECT_GITHUB_SOURCE_NAME.to_string(),
            install_count: None,
            install_url: None,
            created_at: None,
            repo_url: Some("https://github.com/example/demo".to_string()),
            skill_path: Some("skills/demo".to_string()),
            external_url: Some("https://github.com/example/demo/tree/main/skills/demo".to_string()),
            remote_revision: None,
            tags: Vec::new(),
            install_status: InstallStatus::NotInstalled,
            installations: Vec::new(),
            clawhub_slug: None,
            clawhub_owner: None,
            clawhub_version: None,
        };
        let tree = SkillFileNode {
            name: "demo".to_string(),
            path: "skills/demo".to_string(),
            is_dir: true,
            download_url: None,
            sha: None,
            children: Some(vec![SkillFileNode {
                name: "SKILL.md".to_string(),
                path: "skills/demo/SKILL.md".to_string(),
                is_dir: false,
                download_url: Some("https://example.com/demo".to_string()),
                sha: None,
                children: None,
            }]),
        };

        assert!(expand_skill_group_reference(&skill, &tree).is_empty());
    }

    #[test]
    fn collect_installed_marketplace_skills_respects_source_filter_and_query() {
        let skills = vec![
            make_marketplace_skill("src_skills::alpha", "src_skills", "Alpha", Some("useful")),
            make_marketplace_skill("src_other::beta", "src_other", "Beta", Some("other")),
            Skill {
                id: "local-only".to_string(),
                instance_id: Skill::global_instance_id("local-only"),
                scope: sm_core::models::SkillScope::Global,
                project_id: None,
                project_name: None,
                tool_id: None,
                name: "Local".to_string(),
                description: Some("ignore".to_string()),
                version: "1.0.0".to_string(),
                source: SkillSource::Local,
                marketplace_meta: None,
                vault_meta: None,
                package_meta: None,
                contract: crate::models::SkillContractSummary::unmanaged(),
                enabled: HashMap::new(),
                path: PathBuf::from("/tmp/local-only"),
            },
        ];
        let sources = vec![
            make_source("src_skills", true),
            make_source("src_other", true),
        ];
        let source_filter = Some(vec!["src_skills".to_string()]);

        let collected =
            collect_installed_marketplace_skills(&skills, &sources, Some("alp"), &source_filter);

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].id, "src_skills::alpha");
        assert_eq!(collected[0].install_status, InstallStatus::Installed);
        assert_eq!(collected[0].source_name, "src_skills");
    }

    #[test]
    fn prepend_missing_installed_marketplace_skills_prepends_only_missing_entries() {
        let response = MarketplaceSkillsResponse {
            skills: vec![
                make_listing_skill("src_skills::alpha", InstallStatus::UpdateAvailable),
                make_listing_skill("src_skills::gamma", InstallStatus::NotInstalled),
            ],
            has_more: true,
        };

        let merged = prepend_missing_installed_marketplace_skills(
            response,
            vec![
                make_listing_skill("src_skills::beta", InstallStatus::Installed),
                make_listing_skill("src_skills::alpha", InstallStatus::Installed),
            ],
        );

        assert_eq!(
            merged
                .skills
                .iter()
                .map(|skill| skill.id.as_str())
                .collect::<Vec<_>>(),
            vec!["src_skills::beta", "src_skills::alpha", "src_skills::gamma"]
        );
        assert_eq!(
            merged.skills[1].install_status,
            InstallStatus::UpdateAvailable
        );
        assert!(merged.has_more);
    }

    #[test]
    fn only_direct_github_installs_are_hydrated_when_missing_from_listing() {
        let builtin = make_listing_skill("src_skills::alpha", InstallStatus::Installed);
        assert!(
            !should_hydrate_missing_installed_marketplace_skill(&builtin),
            "builtin marketplace skills already have remote metadata in listing and should not block page load"
        );

        let direct = MarketplaceSkill {
            source_id: DIRECT_GITHUB_SOURCE_ID.to_string(),
            source_name: DIRECT_GITHUB_SOURCE_NAME.to_string(),
            repo_url: Some("https://github.com/example/repo".to_string()),
            skill_path: Some("skills/demo".to_string()),
            ..make_listing_skill("github-direct-demo", InstallStatus::Installed)
        };
        assert!(
            should_hydrate_missing_installed_marketplace_skill(&direct),
            "direct GitHub installs still need remote hydration for update tracking"
        );

        let clawhub = MarketplaceSkill {
            source_id: CLAWHUB_SOURCE_ID.to_string(),
            source_name: "ClawHub".to_string(),
            clawhub_slug: Some("demo".to_string()),
            ..make_listing_skill("src_clawhub::demo", InstallStatus::Installed)
        };
        assert!(
            should_hydrate_missing_installed_marketplace_skill(&clawhub),
            "installed ClawHub skills outside the current page need latest-version hydration"
        );
    }
}
