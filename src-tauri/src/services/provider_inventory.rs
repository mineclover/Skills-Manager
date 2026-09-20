use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{
    home_dir, AppConfig, OrcaInventory, Skill, SkillBinding, SkillBindingImpact, SkillBindingState,
    SkillOperationAction, SkillOperationPreview, SkillProvider, SkillProviderCapabilities,
    SkillProviderInventory, SkillProviderKind, SkillScope, SUPPORTED_TOOLS,
};
use crate::services::{
    ConfigManager, DetectorService, LinkStatus, LinkerService, OrcaService, ScannerService,
    WorkspaceService,
};

pub struct ProviderInventoryService;

impl ProviderInventoryService {
    pub fn list(project_id: Option<&str>) -> Result<SkillProviderInventory, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        Ok(Self::list_with_skills(&config, &skills))
    }

    pub fn list_with_skills(config: &AppConfig, skills: &[Skill]) -> SkillProviderInventory {
        let orca = OrcaService::inspect();
        let mut providers = filesystem_providers(config, skills);
        providers.push(orca_provider(&orca));
        providers.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));

        SkillProviderInventory {
            checked_at: current_timestamp(),
            providers,
            orca,
        }
    }

    pub fn list_bindings(
        project_id: Option<&str>,
        provider_id: Option<&str>,
        skill_instance_id: Option<&str>,
    ) -> Result<Vec<SkillBinding>, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        Ok(Self::list_bindings_with_skills(
            &config,
            &skills,
            provider_id,
            skill_instance_id,
        ))
    }

    pub fn list_bindings_with_skills(
        config: &AppConfig,
        skills: &[Skill],
        provider_id: Option<&str>,
        skill_instance_id: Option<&str>,
    ) -> Vec<SkillBinding> {
        let providers = filesystem_providers(config, skills);
        let now = current_timestamp();
        let mut bindings = Vec::new();

        for skill in skills {
            let shared_root = shared_root_for_skill(config, skill);
            let shared_consumers = consumers_for_root(config, skill, &shared_root);
            if skill_instance_id.is_some_and(|value| value != skill.instance_id) {
                continue;
            }

            for provider in &providers {
                if provider_id.is_some_and(|value| value != provider.provider_id) {
                    continue;
                }

                let is_shared = provider.provider_id == "agents-directory";
                let is_direct_for_provider = skill.scope == SkillScope::Tool
                    && skill.tool_id.as_deref() == Some(provider.provider_id.as_str());
                let is_shared_direct = skill.scope == SkillScope::Tool
                    && skill
                        .path
                        .strip_prefix(&shared_root)
                        .map(|_| true)
                        .unwrap_or(false)
                    && shared_consumers
                        .iter()
                        .any(|consumer| skill.tool_id.as_deref() == Some(consumer.as_str()));

                let is_managed_for_provider =
                    skill.scope != SkillScope::Tool
                        && if is_shared {
                            !shared_consumers.is_empty()
                                && (skill.enabled.keys().any(|consumer| {
                                    shared_consumers.iter().any(|id| id == consumer)
                                }) || skill.path.exists())
                        } else {
                            provider.detected || skill.enabled.contains_key(&provider.provider_id)
                        };

                if !(is_direct_for_provider
                    || (is_shared && is_shared_direct)
                    || is_managed_for_provider)
                {
                    continue;
                }

                let binding = if is_shared {
                    shared_binding(skill, provider, &shared_consumers, &shared_root, now)
                } else {
                    filesystem_binding(config, skill, provider, now)
                };
                bindings.push(binding);
            }
        }

        bindings.sort_by(|a, b| {
            a.provider_id
                .cmp(&b.provider_id)
                .then_with(|| a.skill_instance_id.cmp(&b.skill_instance_id))
        });
        bindings
    }

    pub fn ensure_activation_capability(
        config: &AppConfig,
        provider_id: &str,
        enabled: bool,
    ) -> Result<(), String> {
        if matches!(provider_id, "agents-directory" | "orca") {
            return Err(format!(
                "Provider {provider_id} is read-only; select a writable consuming tool"
            ));
        }

        let tool_config = config
            .get_tool_config(provider_id)
            .ok_or_else(|| format!("Provider not found: {provider_id}"))?;
        let capabilities = filesystem_capabilities();
        let supported = if enabled {
            capabilities.enable
        } else {
            capabilities.disable
        };
        if !supported {
            return Err(format!(
                "Provider {provider_id} does not support this activation operation"
            ));
        }

        // A disabled tool can still own a directly installed skill. The
        // operation is therefore gated by provider capability, not detection.
        let _ = tool_config;
        Ok(())
    }

    pub fn preview_binding_operation(
        project_id: Option<&str>,
        skill_instance_id: &str,
        provider_id: &str,
        enabled: bool,
    ) -> Result<SkillOperationPreview, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_skills_for_scope(&config, project_id)?;
        Self::preview_binding_operation_with_skills(
            &config,
            &skills,
            project_id,
            skill_instance_id,
            provider_id,
            enabled,
        )
    }

    pub fn preview_binding_operation_with_skills(
        config: &AppConfig,
        skills: &[Skill],
        _project_id: Option<&str>,
        skill_instance_id: &str,
        provider_id: &str,
        enabled: bool,
    ) -> Result<SkillOperationPreview, String> {
        let skill = skills
            .iter()
            .find(|skill| skill.instance_id == skill_instance_id)
            .ok_or_else(|| format!("Skill not found: {skill_instance_id}"))?;
        let providers = filesystem_providers(config, skills);
        let provider = providers
            .iter()
            .find(|provider| provider.provider_id == provider_id)
            .ok_or_else(|| format!("Provider not found: {provider_id}"))?;
        let target_root = provider_target_root(config, skill, provider);
        let root_consumers = target_root
            .as_ref()
            .map(|root| consumers_for_root(config, skill, root))
            .unwrap_or_default();
        // Tool-local Codex operations only change its configuration. Project
        // bindings and filesystem providers can change the shared directory.
        let config_only = skill.scope == SkillScope::Tool && provider_id == "codex";
        let agents_root = shared_root_for_skill(config, skill);
        let targets_agents_root = target_root
            .as_ref()
            .is_some_and(|root| same_root(root, &agents_root));
        let is_shared = !config_only && (root_consumers.len() > 1 || targets_agents_root);
        let mut impacted_ids = vec![provider_id.to_string()];
        if is_shared {
            for consumer_id in root_consumers {
                if !impacted_ids.contains(&consumer_id) {
                    impacted_ids.push(consumer_id);
                }
            }
            if targets_agents_root && !impacted_ids.iter().any(|id| id == "agents-directory") {
                impacted_ids.push("agents-directory".to_string());
            }
        }
        let will_change = skill.is_enabled_for(provider_id) != enabled;
        let mut impacts = impacted_ids.into_iter().map(|id| {
            let display_name = providers.iter().find(|item| item.provider_id == id)
                .map(|item| item.display_name.clone())
                .or_else(|| SUPPORTED_TOOLS.iter().find(|item| item.id == id).map(|item| item.name.to_string()))
                .unwrap_or_else(|| if id == "agents-directory" { "Shared Agents Directory".to_string() } else { id.clone() });
            let impact_root = if id == provider_id || id == "agents-directory" {
                target_root.clone()
            } else {
                effective_tool_root(config, skill, &id).or_else(|| target_root.clone())
            };
            SkillBindingImpact {
                provider_id: id,
                display_name,
                // Preserve each consumer's configured scoped path, including
                // aliases and indirect bindings in another registered project.
                root_path: impact_root,
                shared: is_shared,
                reason: if !will_change {
                    Some("No change: the binding already has the requested state".to_string())
                } else if is_shared {
                    Some("The target skill directory is shared with these agent consumers".to_string())
                } else if config_only {
                    Some("Only the selected Codex configuration changes; skill files remain in place".to_string())
                } else { None },
            }
        }).collect::<Vec<_>>();
        if will_change && !config_only {
            if let Some(root) = &target_root {
                for impact in
                    known_binding_impacts(config, skill, provider_id, enabled, root, &providers)?
                {
                    if !impacts.iter().any(|existing| {
                        existing.provider_id == impact.provider_id
                            && existing.root_path == impact.root_path
                    }) {
                        impacts.push(impact);
                    }
                }
                if impacts.len() > 1 {
                    for impact in &mut impacts {
                        impact.shared = true;
                    }
                }
            }
        }
        let requires_confirmation = will_change && impacts.iter().any(|impact| impact.shared);
        let warning = requires_confirmation.then(|| {
            format!(
                "This operation may affect: {}",
                impacts
                    .iter()
                    .map(|impact| {
                        format!(
                            "{} ({})",
                            impact.display_name,
                            impact
                                .root_path
                                .as_ref()
                                .map(|root| root.display().to_string())
                                .unwrap_or_else(|| "unknown root".to_string())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });

        Ok(SkillOperationPreview {
            skill_instance_id: skill.instance_id.clone(),
            artifact_id: skill.id.clone(),
            provider_id: provider_id.to_string(),
            scope: skill.scope.clone(),
            action: if enabled {
                SkillOperationAction::Enable
            } else {
                SkillOperationAction::Disable
            },
            target_root,
            impacts,
            requires_confirmation,
            warning,
        })
    }
}

fn path_is_present(path: &std::path::Path) -> bool {
    path.exists() || path.symlink_metadata().is_ok()
}

fn shared_root_for_skill(config: &AppConfig, skill: &Skill) -> PathBuf {
    if skill.scope == SkillScope::Project {
        if let Some(root) = skill
            .project_id
            .as_deref()
            .and_then(|id| config.projects.iter().find(|project| project.id == id))
            .and_then(|project| project.root_path.as_ref())
        {
            return root.join(".agents").join("skills");
        }
    }
    shared_agents_skills_path()
}

fn effective_tool_root(config: &AppConfig, skill: &Skill, tool_id: &str) -> Option<PathBuf> {
    if skill.scope == SkillScope::Project {
        if let Some(project) = skill
            .project_id
            .as_deref()
            .and_then(|id| config.projects.iter().find(|project| project.id == id))
        {
            if let Some(root) = WorkspaceService::project_tool_skills_dir(project, tool_id) {
                return Some(root);
            }
        }
    }
    config.get_tool_config(tool_id).map(|tool| tool.skills_path)
}

fn canonical_root(root: &std::path::Path) -> PathBuf {
    // Compare physical directories even before a not-yet-installed skills leaf
    // exists (for example /var and /private/var aliases on macOS).
    if let Ok(path) = std::fs::canonicalize(root) {
        return path;
    }
    match (root.parent(), root.file_name()) {
        (Some(parent), Some(name)) => canonical_root(parent).join(name),
        _ => root.to_path_buf(),
    }
}

fn same_root(left: &std::path::Path, right: &std::path::Path) -> bool {
    canonical_root(left) == canonical_root(right)
}

fn consumers_for_root(config: &AppConfig, skill: &Skill, root: &std::path::Path) -> Vec<String> {
    let mut consumers = config
        .collect_tool_configs()
        .into_iter()
        .filter_map(|(id, _)| {
            effective_tool_root(config, skill, &id)
                .filter(|candidate| same_root(candidate, root))
                .map(|_| id)
        })
        .collect::<Vec<_>>();
    consumers.sort();
    consumers.dedup();
    consumers
}

fn absolute_lexical_path(path: &std::path::Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

// Follow only a known binding and its symlink chain. Never enumerate user
// directories. Detect the source path before resolving its final symlink so
// renaming an alias does not implicate independent links to its real target.
fn binding_depends_on_renamed_path(
    binding: &std::path::Path,
    renamed_paths: &[PathBuf],
) -> Result<(bool, bool), String> {
    let mut watched = Vec::new();
    for path in renamed_paths {
        watched.push(absolute_lexical_path(path));
        if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
            watched.push(absolute_lexical_path(&canonical_root(parent).join(name)));
        }
    }
    let mut current = absolute_lexical_path(binding);
    let mut visited = HashSet::new();
    let mut dependent = false;
    for _ in 0..64 {
        if !visited.insert(current.clone()) {
            return Err(format!(
                "Cannot safely inspect known binding {}: symlink cycle",
                binding.display()
            ));
        }
        let components = current
            .components()
            .map(|component| component.as_os_str().to_owned())
            .collect::<Vec<_>>();
        let mut prefix = PathBuf::new();
        let mut followed = false;
        for (index, component) in components.iter().enumerate() {
            prefix.push(component);
            if watched.iter().any(|path| path == &prefix) {
                dependent = true;
            }
            let metadata = match std::fs::symlink_metadata(&prefix) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok((dependent, true))
                }
                Err(error) => {
                    return Err(format!(
                        "Cannot safely inspect known binding {}: {error}",
                        binding.display()
                    ))
                }
            };
            if metadata.file_type().is_symlink() {
                let target = std::fs::read_link(&prefix).map_err(|error| {
                    format!("Cannot read known binding {}: {error}", binding.display())
                })?;
                let mut resolved = if target.is_absolute() {
                    target
                } else {
                    prefix
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(target)
                };
                for remaining in &components[index + 1..] {
                    resolved.push(remaining);
                }
                current = absolute_lexical_path(&resolved);
                followed = true;
                break;
            }
        }
        if !followed {
            return Ok((dependent, false));
        }
    }
    Err(format!(
        "Cannot safely inspect known binding {}: symlink chain exceeds 64 hops",
        binding.display()
    ))
}

fn known_binding_impacts(
    config: &AppConfig,
    skill: &Skill,
    provider_id: &str,
    enabled: bool,
    target_root: &std::path::Path,
    providers: &[SkillProvider],
) -> Result<Vec<SkillBindingImpact>, String> {
    let enabled_path = target_root.join(&skill.id);
    let disabled_path = target_root.join(format!(
        "{}{}",
        skill.id,
        crate::models::DISABLED_TOOL_SKILL_SUFFIX
    ));
    let renames_source = (skill.scope == SkillScope::Tool && provider_id != "codex")
        || (!enabled && skill.path == enabled_path)
        || (enabled && skill.path == disabled_path);
    let renamed_paths = if renames_source {
        vec![
            skill.path.clone(),
            if enabled { enabled_path } else { disabled_path },
        ]
    } else {
        Vec::new()
    };
    let mut roots = Vec::new();
    for (id, tool) in config.collect_tool_configs() {
        roots.push((id.clone(), tool.skills_path.clone()));
        for project in &config.projects {
            if let Some(root) = WorkspaceService::project_tool_skills_dir(project, &id) {
                roots.push((id.clone(), root));
            }
        }
    }
    roots.sort();
    roots.dedup();
    let mut impacts = Vec::new();
    for (id, root) in roots {
        let same_directory = same_root(&root, target_root);
        let mut indirect = false;
        let mut broken = false;
        if !same_directory && renames_source {
            for name in [
                skill.id.clone(),
                format!("{}{}", skill.id, crate::models::DISABLED_TOOL_SKILL_SUFFIX),
            ] {
                let (depends, missing) =
                    binding_depends_on_renamed_path(&root.join(name), &renamed_paths)?;
                indirect |= depends;
                broken |= depends && missing;
            }
        }
        if !same_directory && !indirect {
            continue;
        }
        let display_name = providers
            .iter()
            .find(|provider| provider.provider_id == id)
            .map(|provider| provider.display_name.clone())
            .or_else(|| {
                SUPPORTED_TOOLS
                    .iter()
                    .find(|definition| definition.id == id)
                    .map(|definition| definition.name.to_string())
            })
            .unwrap_or_else(|| id.clone());
        impacts.push(SkillBindingImpact {
            provider_id: id,
            display_name,
            root_path: Some(root),
            shared: true,
            reason: Some(if indirect {
                format!(
                    "Indirect source dependency: this binding follows {} which will be renamed{}",
                    skill.path.display(),
                    if broken {
                        "; the dependent target is currently broken"
                    } else {
                        ""
                    }
                )
            } else {
                "This known binding root refers to the same physical skill directory".to_string()
            }),
        });
    }
    Ok(impacts)
}

fn provider_target_root(
    config: &AppConfig,
    skill: &Skill,
    provider: &SkillProvider,
) -> Option<PathBuf> {
    if provider.provider_id == "agents-directory" {
        Some(shared_root_for_skill(config, skill))
    } else {
        effective_tool_root(config, skill, &provider.provider_id)
            .or_else(|| provider.root_path.clone())
    }
}

fn filesystem_binding(
    config: &AppConfig,
    skill: &Skill,
    provider: &SkillProvider,
    checked_at: u64,
) -> SkillBinding {
    let is_direct = skill.scope == SkillScope::Tool;
    let expected_enabled = skill.is_enabled_for(&provider.provider_id);
    let target_root = provider_target_root(config, skill, provider);
    let target_path = if is_direct {
        Some(skill.path.clone())
    } else {
        target_root.as_ref().map(|root| root.join(&skill.id))
    };
    let directly_registered = !is_direct && target_path.as_ref() == Some(&skill.path);
    let has_project_target = skill.scope == SkillScope::Project && target_root.is_some();
    let source_path = Some(skill.path.clone());
    let source_exists = path_is_present(&skill.path);

    let (state, reason) = if is_direct {
        if expected_enabled && source_exists {
            (SkillBindingState::Enabled, None)
        } else if !expected_enabled && source_exists {
            (
                SkillBindingState::Disabled,
                Some("Direct skill is disabled at its provider path".to_string()),
            )
        } else {
            (
                SkillBindingState::Missing,
                Some("Direct skill path is no longer present".to_string()),
            )
        }
    } else if directly_registered && expected_enabled {
        (SkillBindingState::Enabled, None)
    } else if !has_project_target && (!provider.detected || provider.reachable == Some(false)) {
        (
            SkillBindingState::Unavailable,
            Some("Provider path is not detected or reachable".to_string()),
        )
    } else if let Some(target_path) = &target_path {
        let link_status = LinkerService::check_link_for_scoped_skill(
            &skill.path,
            target_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            &skill.id,
            &provider.provider_id,
            &skill.scope,
        );
        binding_state_for_link(expected_enabled, link_status)
    } else {
        (
            SkillBindingState::Unavailable,
            Some("Provider has no filesystem target".to_string()),
        )
    };

    SkillBinding {
        artifact_id: skill.id.clone(),
        skill_instance_id: skill.instance_id.clone(),
        provider_id: provider.provider_id.clone(),
        scope: skill.scope.clone(),
        state,
        source_path,
        target_path,
        last_checked_at: checked_at,
        reason,
    }
}

fn shared_binding(
    skill: &Skill,
    provider: &SkillProvider,
    consumers: &[String],
    shared_root: &std::path::Path,
    checked_at: u64,
) -> SkillBinding {
    let direct = skill.scope == SkillScope::Tool || shared_root.join(&skill.id) == skill.path;
    let target_path = if direct {
        Some(skill.path.clone())
    } else {
        Some(shared_root.join(&skill.id))
    };
    let source_exists = path_is_present(&skill.path);
    let (state, reason) = if direct {
        let expected_enabled = skill
            .tool_id
            .as_deref()
            .map(|tool_id| skill.is_enabled_for(tool_id))
            .unwrap_or_else(|| consumers.iter().any(|id| skill.is_enabled_for(id)));
        if expected_enabled && source_exists {
            (SkillBindingState::Enabled, None)
        } else if !expected_enabled && source_exists {
            (
                SkillBindingState::Disabled,
                Some("Direct skill is disabled at the shared agents path".to_string()),
            )
        } else {
            (
                SkillBindingState::Missing,
                Some("Direct skill path is no longer present".to_string()),
            )
        }
    } else if !shared_root.exists() {
        (
            SkillBindingState::Unavailable,
            Some("Shared agents directory is not reachable".to_string()),
        )
    } else {
        let expectations = consumers
            .iter()
            .filter_map(|consumer| {
                skill
                    .enabled
                    .get(consumer)
                    .map(|enabled| (consumer, *enabled))
            })
            .collect::<Vec<_>>();
        let expected_enabled = expectations.iter().any(|(_, enabled)| *enabled);
        let expectation_conflict = expectations
            .first()
            .map(|(_, enabled)| expectations.iter().any(|(_, value)| value != enabled))
            .unwrap_or(false);
        let checker_tool_id = expectations
            .first()
            .map(|(consumer, _)| consumer.as_str())
            .or_else(|| consumers.first().map(String::as_str))
            .unwrap_or("agents-directory");
        let link_status = LinkerService::check_link_for_scoped_skill(
            &skill.path,
            shared_root,
            &skill.id,
            checker_tool_id,
            &skill.scope,
        );
        let (link_state, link_reason) = binding_state_for_link(expected_enabled, link_status);
        if expectation_conflict {
            (
                SkillBindingState::Conflict,
                Some("Shared directory consumers have different enabled states".to_string()),
            )
        } else {
            (link_state, link_reason)
        }
    };

    SkillBinding {
        artifact_id: skill.id.clone(),
        skill_instance_id: skill.instance_id.clone(),
        provider_id: provider.provider_id.clone(),
        scope: skill.scope.clone(),
        state,
        source_path: Some(skill.path.clone()),
        target_path,
        last_checked_at: checked_at,
        reason,
    }
}

fn binding_state_for_link(
    expected_enabled: bool,
    link_status: LinkStatus,
) -> (SkillBindingState, Option<String>) {
    match (expected_enabled, link_status) {
        (true, LinkStatus::Valid) => (SkillBindingState::Enabled, None),
        (false, LinkStatus::Missing) => (
            SkillBindingState::Disabled,
            Some("Binding is disabled and no target is present".to_string()),
        ),
        (true, LinkStatus::Missing) => (
            SkillBindingState::Missing,
            Some("Expected binding target is missing".to_string()),
        ),
        (false, LinkStatus::Valid) => (
            SkillBindingState::Conflict,
            Some("Target exists while the binding is disabled".to_string()),
        ),
        (_, LinkStatus::Broken) => (
            SkillBindingState::Missing,
            Some("Binding target is broken".to_string()),
        ),
        (_, LinkStatus::WrongTarget) => (
            SkillBindingState::Conflict,
            Some("Binding target points to another skill".to_string()),
        ),
        (_, LinkStatus::NotALink) => (
            SkillBindingState::Conflict,
            Some("Binding target is not a managed link".to_string()),
        ),
    }
}

fn filesystem_providers(config: &AppConfig, skills: &[Skill]) -> Vec<SkillProvider> {
    let builtin_names: HashMap<&str, (&str, &str)> = SUPPORTED_TOOLS
        .iter()
        .map(|definition| (definition.id, (definition.name, definition.cli_command)))
        .collect();
    let mut providers = Vec::new();

    for (provider_id, tool_config) in config.collect_tool_configs() {
        let (display_name, cli_command) = builtin_names
            .get(provider_id.as_str())
            .map(|(name, command)| ((*name).to_string(), Some(*command)))
            .unwrap_or_else(|| {
                (
                    config
                        .custom_tools
                        .get(&provider_id)
                        .map(|tool| tool.name.clone())
                        .unwrap_or_else(|| provider_id.clone()),
                    None,
                )
            });
        let cli_available = cli_command
            .map(DetectorService::check_cli_available)
            .unwrap_or(false);
        let provider_skills = skills.iter().filter(|skill| {
            skill.scope != SkillScope::Tool
                || skill.tool_id.as_deref() == Some(provider_id.as_str())
        });
        let provider_skills = provider_skills.collect::<Vec<_>>();
        let skill_count = provider_skills.len();
        let enabled_count = provider_skills
            .iter()
            .filter(|skill| skill.is_enabled_for(&provider_id))
            .count();
        let detected = tool_config.detected
            || tool_config.config_path.exists()
            || tool_config.skills_path.exists();
        let direct_skill_count = skills
            .iter()
            .filter(|skill| {
                skill.scope == SkillScope::Tool
                    && skill.tool_id.as_deref() == Some(provider_id.as_str())
            })
            .count();

        let has_managed_binding = skills.iter().any(|skill| {
            skill.scope != SkillScope::Tool && skill.enabled.contains_key(&provider_id)
        });
        if !detected && !tool_config.enabled && direct_skill_count == 0 && !has_managed_binding {
            continue;
        }

        let warning = if direct_skill_count > 0 && !tool_config.enabled {
            Some("Provider is disabled; direct skills remain visible and actionable".to_string())
        } else if provider_id == "vercel-skills"
            && tool_config.skills_path == shared_agents_skills_path()
        {
            Some(
                "This path is shared with the agents directory and may affect multiple agents"
                    .to_string(),
            )
        } else {
            None
        };

        providers.push(SkillProvider {
            provider_id: provider_id.clone(),
            kind: if provider_id == "codex" {
                SkillProviderKind::ConfigFile
            } else {
                SkillProviderKind::Filesystem
            },
            display_name,
            root_path: Some(tool_config.skills_path.clone()),
            detected,
            cli_available,
            reachable: Some(detected),
            capabilities: filesystem_capabilities(),
            skill_count,
            enabled_count,
            disabled_count: skill_count.saturating_sub(enabled_count),
            warning,
        });
    }

    let shared_root = shared_agents_skills_path();
    let shared_skills = skills
        .iter()
        .filter(|skill| {
            skill
                .path
                .strip_prefix(shared_root_for_skill(config, skill))
                .is_ok()
        })
        .collect::<Vec<_>>();
    let mut shared_consumers = config
        .collect_tool_configs()
        .into_iter()
        .filter(|(_, tool)| same_root(&tool.skills_path, &shared_root))
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for skill in skills {
        shared_consumers.extend(consumers_for_root(
            config,
            skill,
            &shared_root_for_skill(config, skill),
        ));
    }
    shared_consumers.sort();
    shared_consumers.dedup();
    let mut observed_roots = skills
        .iter()
        .map(|skill| shared_root_for_skill(config, skill))
        .filter(|root| root.exists())
        .collect::<Vec<_>>();
    if shared_root.exists() {
        observed_roots.push(shared_root.clone());
    }
    observed_roots.sort();
    observed_roots.dedup();
    if !observed_roots.is_empty() || !shared_skills.is_empty() {
        let warning = if shared_consumers.is_empty() {
            "Shared directory is outside the configured tool list".to_string()
        } else {
            format!(
                "Shared directory may affect: {}",
                shared_consumers.join(", ")
            )
        };
        let enabled_count = shared_skills
            .iter()
            .filter(|skill| skill.enabled.values().any(|enabled| *enabled))
            .count();
        providers.push(SkillProvider {
            provider_id: "agents-directory".to_string(),
            kind: SkillProviderKind::Filesystem,
            display_name: "Shared Agents Directory".to_string(),
            root_path: if observed_roots.len() == 1 {
                observed_roots.first().cloned()
            } else {
                None
            },
            detected: true,
            cli_available: false,
            reachable: Some(true),
            capabilities: SkillProviderCapabilities {
                list: true,
                inspect: true,
                ..SkillProviderCapabilities::default()
            },
            skill_count: shared_skills.len(),
            enabled_count,
            disabled_count: shared_skills.len().saturating_sub(enabled_count),
            warning: Some(warning),
        });
    }

    providers
}

fn orca_provider(orca: &OrcaInventory) -> SkillProvider {
    SkillProvider {
        provider_id: "orca".to_string(),
        kind: SkillProviderKind::Cli,
        display_name: "Orca".to_string(),
        root_path: None,
        detected: orca.cli_available,
        cli_available: orca.cli_available,
        reachable: orca.runtime_reachable,
        capabilities: SkillProviderCapabilities {
            list: true,
            inspect: true,
            ..SkillProviderCapabilities::default()
        },
        skill_count: orca.topics.len(),
        enabled_count: 0,
        disabled_count: 0,
        warning: orca.warning.clone(),
    }
}

fn filesystem_capabilities() -> SkillProviderCapabilities {
    SkillProviderCapabilities {
        list: true,
        install: true,
        enable: true,
        disable: true,
        inspect: true,
        ..SkillProviderCapabilities::default()
    }
}

pub(crate) fn shared_agents_skills_path() -> PathBuf {
    home_dir()
        .unwrap_or_default()
        .join(".agents")
        .join("skills")
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{filesystem_providers, ProviderInventoryService};
    use crate::models::{AppConfig, ProjectBinding, Skill, SkillScope, SkillSource, ToolConfig};
    use crate::test_support::with_temp_home;
    use std::fs;

    fn config_for(home: &std::path::Path) -> AppConfig {
        let mut config = AppConfig::default();
        config.initialized = true;
        config.skills_dir = home.join(".skills-manager").join("skills");
        config.tools.insert(
            "claude-code".to_string(),
            ToolConfig {
                enabled: false,
                detected: false,
                skills_path: home.join(".claude").join("skills"),
                config_path: home.join(".claude"),
            },
        );
        config
    }

    fn direct_skill(home: &std::path::Path) -> Skill {
        let path = home
            .join(".claude")
            .join("skills")
            .join("direct-disabled.disabled-by-sm");
        Skill {
            id: "direct-disabled".to_string(),
            instance_id: Skill::tool_instance_id("claude-code", "direct-disabled"),
            scope: SkillScope::Tool,
            project_id: None,
            project_name: None,
            tool_id: Some("claude-code".to_string()),
            name: "direct-disabled".to_string(),
            description: None,
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            marketplace_meta: None,
            vault_meta: None,
            package_meta: None,
            contract: crate::models::SkillContractSummary::unmanaged(),
            enabled: HashMap::from([(String::from("claude-code"), false)]),
            path,
        }
    }

    fn project_fixture(
        home: &std::path::Path,
        direct: bool,
    ) -> (AppConfig, Skill, std::path::PathBuf) {
        let repository = home.join("project-a");
        let root = repository.join(".agents").join("skills");
        let source = if direct {
            root.clone()
        } else {
            repository.join("skills")
        }
        .join("shared-skill");
        fs::create_dir_all(&source).unwrap();
        let mut config = config_for(home);
        for (id, directory) in [
            ("codex", ".codex"),
            ("vercel-skills", ".agents"),
            ("claude-code", ".claude"),
        ] {
            config.tools.insert(
                id.to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: home.join(directory).join("skills"),
                    config_path: home.join(directory),
                },
            );
        }
        for name in ["project-a", "project-b"] {
            config.projects.push(ProjectBinding {
                id: name.to_string(),
                name: name.to_string(),
                skills_dir: home.join(name).join("skills"),
                root_path: Some(home.join(name)),
            });
        }
        let mut skill = Skill::new(
            "shared-skill".to_string(),
            "Shared skill".to_string(),
            source,
        )
        .with_scope(
            SkillScope::Project,
            Some("project-a".to_string()),
            Some("Project A".to_string()),
        );
        if direct {
            skill.enabled.insert("codex".to_string(), true);
            skill.enabled.insert("vercel-skills".to_string(), true);
        }
        (config, skill, root)
    }

    #[test]
    fn project_shared_preview_uses_effective_consumer_roots_and_isolates_other_projects() {
        with_temp_home(|home| {
            let (config, skill, root) = project_fixture(home, false);
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                true,
            )
            .unwrap();
            assert!(preview.requires_confirmation);
            let ids = preview
                .impacts
                .iter()
                .map(|impact| impact.provider_id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(ids, vec!["codex", "vercel-skills", "agents-directory"]);
            assert!(preview
                .impacts
                .iter()
                .all(|impact| impact.shared && impact.root_path.as_ref() == Some(&root)));
            assert!(!preview.warning.as_ref().unwrap().contains("project-b"));
            assert!(!preview
                .impacts
                .iter()
                .any(|impact| impact.root_path.as_ref()
                    == Some(&home.join(".agents").join("skills"))));
        });
    }

    #[cfg(unix)]
    #[test]
    fn direct_source_rename_lists_known_indirect_bindings_with_their_own_project_roots() {
        with_temp_home(|home| {
            let (config, skill, target_root) = project_fixture(home, true);
            for project in ["project-a", "project-b"] {
                let root = home.join(project).join(".claude").join("skills");
                fs::create_dir_all(&root).unwrap();
                std::os::unix::fs::symlink(&skill.path, root.join(&skill.id)).unwrap();
            }
            let other_codex_root = home.join("project-b").join(".agents").join("skills");
            fs::create_dir_all(&other_codex_root).unwrap();
            std::os::unix::fs::symlink(&skill.path, other_codex_root.join(&skill.id)).unwrap();
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                false,
            )
            .unwrap();
            assert_eq!(preview.target_root, Some(target_root));
            assert!(preview
                .impacts
                .iter()
                .any(|impact| impact.provider_id == "codex"
                    && impact.root_path.as_ref() == Some(&other_codex_root)));
            let mut serialized = serde_json::to_value(&preview).unwrap();
            assert!(serialized["target_root"].is_string());
            serialized.as_object_mut().unwrap().remove("target_root");
            let legacy: crate::models::SkillOperationPreview =
                serde_json::from_value(serialized).unwrap();
            assert_eq!(legacy.target_root, None);
            let indirect = preview
                .impacts
                .iter()
                .filter(|impact| impact.provider_id == "claude-code")
                .collect::<Vec<_>>();
            assert_eq!(indirect.len(), 2);
            for project in ["project-a", "project-b"] {
                assert!(indirect.iter().any(|impact| impact.root_path
                    == Some(home.join(project).join(".claude").join("skills"))));
            }
            assert!(indirect.iter().all(|impact| impact.shared
                && impact
                    .reason
                    .as_ref()
                    .unwrap()
                    .contains("Indirect source dependency")));
            assert!(preview.requires_confirmation);
            assert!(!preview
                .impacts
                .iter()
                .any(|impact| impact.root_path == Some(home.join(".claude").join("skills"))));
        });
    }

    #[cfg(unix)]
    #[test]
    fn restoring_a_renamed_source_reports_currently_broken_indirect_bindings() {
        with_temp_home(|home| {
            let (config, mut skill, _) = project_fixture(home, true);
            let root = home.join("project-a").join(".claude").join("skills");
            fs::create_dir_all(&root).unwrap();
            std::os::unix::fs::symlink(&skill.path, root.join(&skill.id)).unwrap();
            let disabled = skill.path.with_file_name("shared-skill.disabled-by-sm");
            fs::rename(&skill.path, &disabled).unwrap();
            skill.path = disabled;
            skill.enabled.insert("codex".to_string(), false);
            skill.enabled.insert("vercel-skills".to_string(), false);
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                true,
            )
            .unwrap();
            let indirect = preview
                .impacts
                .iter()
                .find(|impact| impact.provider_id == "claude-code")
                .unwrap();
            assert_eq!(indirect.root_path, Some(root));
            assert!(indirect
                .reason
                .as_ref()
                .unwrap()
                .contains("currently broken"));
            assert!(preview.requires_confirmation);
        });
    }

    #[cfg(unix)]
    #[test]
    fn known_binding_cycles_fail_closed_for_mutations_but_do_not_block_noops() {
        with_temp_home(|home| {
            let (config, skill, _) = project_fixture(home, true);
            let root = home.join("project-a").join(".claude").join("skills");
            fs::create_dir_all(&root).unwrap();
            std::os::unix::fs::symlink("cycle", root.join(&skill.id)).unwrap();
            std::os::unix::fs::symlink(&skill.id, root.join("cycle")).unwrap();
            let error = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                false,
            )
            .unwrap_err();
            assert!(error.contains("symlink cycle"));
            let unchanged = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                true,
            )
            .unwrap();
            assert!(!unchanged.requires_confirmation);
        });
    }

    #[cfg(unix)]
    #[test]
    fn renaming_a_source_alias_does_not_implicate_independent_links_to_its_real_target() {
        with_temp_home(|home| {
            let (config, skill, _) = project_fixture(home, true);
            let actual = home.join("independent-source");
            fs::create_dir_all(&actual).unwrap();
            fs::remove_dir(&skill.path).unwrap();
            std::os::unix::fs::symlink(&actual, &skill.path).unwrap();
            let root = home.join("project-a").join(".claude").join("skills");
            fs::create_dir_all(&root).unwrap();
            std::os::unix::fs::symlink(&actual, root.join(&skill.id)).unwrap();
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                false,
            )
            .unwrap();
            assert!(!preview
                .impacts
                .iter()
                .any(|impact| impact.provider_id == "claude-code"));
        });
    }

    #[test]
    fn project_noop_keeps_root_information_without_requesting_confirmation() {
        with_temp_home(|home| {
            let (config, skill, root) = project_fixture(home, true);
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "codex",
                true,
            )
            .unwrap();
            assert!(!preview.requires_confirmation);
            assert!(preview.warning.is_none());
            assert!(preview
                .impacts
                .iter()
                .all(|impact| impact.shared && impact.root_path.as_ref() == Some(&root)));
            assert!(preview.impacts[0]
                .reason
                .as_ref()
                .unwrap()
                .starts_with("No change:"));
            let bindings = ProviderInventoryService::list_bindings_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("agents-directory"),
                Some(&skill.instance_id),
            );
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].target_path, Some(root.join("shared-skill")));
            assert_eq!(bindings[0].state, crate::models::SkillBindingState::Enabled);
        });
    }

    #[test]
    fn a_shared_source_does_not_make_an_independent_target_a_shared_mutation() {
        with_temp_home(|home| {
            let (config, skill, _) = project_fixture(home, true);
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("project-a"),
                &skill.instance_id,
                "claude-code",
                true,
            )
            .unwrap();
            assert!(!preview.requires_confirmation);
            assert_eq!(preview.impacts.len(), 1);
            assert!(!preview.impacts[0].shared);
            assert_eq!(
                preview.impacts[0].root_path,
                Some(home.join("project-a").join(".claude").join("skills"))
            );
        });
    }

    #[test]
    fn global_shared_preview_does_not_substitute_project_roots() {
        with_temp_home(|home| {
            let (config, mut skill, _) = project_fixture(home, false);
            skill.scope = SkillScope::Global;
            skill.project_id = None;
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                None,
                &skill.instance_id,
                "vercel-skills",
                true,
            )
            .unwrap();
            assert!(preview.requires_confirmation);
            assert_eq!(
                preview
                    .impacts
                    .iter()
                    .map(|impact| impact.provider_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["vercel-skills", "agents-directory"]
            );
            assert!(preview
                .impacts
                .iter()
                .all(|impact| impact.root_path == Some(home.join(".agents").join("skills"))));
        });
    }

    #[test]
    fn codex_tool_config_only_change_does_not_claim_shared_filesystem_mutation() {
        with_temp_home(|home| {
            let (mut config, mut skill, root) = project_fixture(home, true);
            config.tools.get_mut("codex").unwrap().skills_path = root.clone();
            config.tools.get_mut("vercel-skills").unwrap().skills_path = root;
            skill.scope = SkillScope::Tool;
            skill.tool_id = Some("codex".to_string());
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                None,
                &skill.instance_id,
                "codex",
                false,
            )
            .unwrap();
            assert!(!preview.requires_confirmation);
            assert_eq!(preview.impacts.len(), 1);
            assert!(!preview.impacts[0].shared);
            assert!(preview.impacts[0]
                .reason
                .as_ref()
                .unwrap()
                .contains("configuration changes"));
        });
    }

    #[cfg(unix)]
    #[test]
    fn physical_root_aliases_share_impacts_even_when_the_skills_leaf_is_missing() {
        with_temp_home(|home| {
            let (mut config, mut skill, _) = project_fixture(home, false);
            let physical = home.join("physical");
            let alias = home.join("alias");
            fs::create_dir_all(&physical).unwrap();
            std::os::unix::fs::symlink(&physical, &alias).unwrap();
            config.tools.get_mut("codex").unwrap().skills_path = physical.join("skills");
            config.tools.get_mut("vercel-skills").unwrap().skills_path = alias.join("skills");
            skill.scope = SkillScope::Global;
            skill.project_id = None;
            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                None,
                &skill.instance_id,
                "codex",
                true,
            )
            .unwrap();
            assert!(preview.requires_confirmation);
            assert_eq!(
                preview
                    .impacts
                    .iter()
                    .map(|impact| impact.provider_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["codex", "vercel-skills"]
            );
            assert_eq!(preview.impacts[0].root_path, Some(physical.join("skills")));
            assert_eq!(preview.impacts[1].root_path, Some(alias.join("skills")));
        });
    }

    #[test]
    fn includes_disabled_tool_provider_and_shared_agents_directory() {
        with_temp_home(|home| {
            let config = config_for(home);
            fs::create_dir_all(home.join(".claude").join("skills")).expect("claude skills");
            fs::create_dir_all(home.join(".agents").join("skills")).expect("agents skills");
            let providers = filesystem_providers(&config, &[direct_skill(home)]);

            let claude = providers
                .iter()
                .find(|provider| provider.provider_id == "claude-code")
                .expect("disabled provider with direct skill should be listed");
            assert_eq!(claude.skill_count, 1);
            assert!(claude.warning.is_some());
            assert!(providers
                .iter()
                .any(|provider| provider.provider_id == "agents-directory"));
        });
    }

    #[test]
    fn does_not_include_unobserved_filesystem_provider() {
        with_temp_home(|home| {
            let config = config_for(home);
            let providers = filesystem_providers(&config, &[]);
            assert!(providers
                .iter()
                .all(|provider| provider.provider_id != "claude-code"));
        });
    }

    #[test]
    fn reports_disabled_direct_skill_binding() {
        with_temp_home(|home| {
            let config = config_for(home);
            let skill = direct_skill(home);
            fs::create_dir_all(&skill.path).expect("direct skill path");

            let bindings = ProviderInventoryService::list_bindings_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("claude-code"),
                None,
            );
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                bindings[0].state,
                crate::models::SkillBindingState::Disabled
            );
            assert_eq!(bindings[0].skill_instance_id, skill.instance_id);
        });
    }

    #[test]
    fn reports_missing_managed_binding() {
        with_temp_home(|home| {
            let mut config = config_for(home);
            let skills_dir = home.join(".skills-manager").join("skills");
            let skill_path = skills_dir.join("managed-missing");
            fs::create_dir_all(&skill_path).expect("managed skill path");
            config.tools.insert(
                "claude-code".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: home.join(".claude").join("skills"),
                    config_path: home.join(".claude"),
                },
            );
            fs::create_dir_all(home.join(".claude").join("skills")).expect("claude skills path");

            let mut skill = Skill::new(
                "managed-missing".to_string(),
                "managed-missing".to_string(),
                skill_path,
            );
            skill.enabled.insert("claude-code".to_string(), true);

            let bindings = ProviderInventoryService::list_bindings_with_skills(
                &config,
                std::slice::from_ref(&skill),
                Some("claude-code"),
                None,
            );
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].state, crate::models::SkillBindingState::Missing);
        });
    }

    #[test]
    fn previews_shared_root_impact_and_rejects_read_only_provider_mutation() {
        with_temp_home(|home| {
            let shared_root = home.join(".agents").join("skills");
            fs::create_dir_all(&shared_root).expect("shared root");
            let mut config = config_for(home);
            config.tools.insert(
                "vercel-skills".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: shared_root.clone(),
                    config_path: home.join(".config").join("vercel-skills"),
                },
            );
            let skill_path = home
                .join(".skills-manager")
                .join("skills")
                .join("shared-managed");
            fs::create_dir_all(&skill_path).expect("managed skill");
            let mut skill = Skill::new(
                "shared-managed".to_string(),
                "shared-managed".to_string(),
                skill_path,
            );
            skill.enabled.insert("vercel-skills".to_string(), true);

            let preview = ProviderInventoryService::preview_binding_operation_with_skills(
                &config,
                std::slice::from_ref(&skill),
                None,
                &skill.instance_id,
                "vercel-skills",
                false,
            )
            .expect("shared preview");
            assert!(preview.requires_confirmation);
            assert!(preview
                .impacts
                .iter()
                .any(|impact| impact.provider_id == "agents-directory"));
            assert!(ProviderInventoryService::ensure_activation_capability(
                &config,
                "agents-directory",
                true,
            )
            .is_err());
        });
    }
}
