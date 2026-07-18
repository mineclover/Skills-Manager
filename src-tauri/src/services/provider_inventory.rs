use std::collections::HashMap;
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
    pub fn list() -> Result<SkillProviderInventory, String> {
        let config = ConfigManager::new().load()?;
        let skills = ScannerService::scan_scoped_skills(&config)?;
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
        let shared_root = shared_agents_skills_path();
        let shared_consumers = config
            .collect_tool_configs()
            .into_iter()
            .filter(|(_, tool)| tool.skills_path == shared_root)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let now = current_timestamp();
        let mut bindings = Vec::new();

        for skill in skills {
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
        let shared_root = shared_agents_skills_path();
        let is_shared = target_root.as_ref() == Some(&shared_root)
            || skill.path.strip_prefix(&shared_root).is_ok();

        let mut impacted_ids = Vec::new();
        if is_shared {
            impacted_ids.push(provider_id.to_string());
            for (consumer_id, tool_config) in config.collect_tool_configs() {
                if tool_config.skills_path == shared_root && !impacted_ids.contains(&consumer_id) {
                    impacted_ids.push(consumer_id);
                }
            }
            if !impacted_ids.iter().any(|id| id == "agents-directory") {
                impacted_ids.push("agents-directory".to_string());
            }
        } else {
            impacted_ids.push(provider_id.to_string());
        }

        let impacts = impacted_ids
            .into_iter()
            .map(|id| {
                let inventory_provider = providers.iter().find(|item| item.provider_id == id);
                SkillBindingImpact {
                    provider_id: id.clone(),
                    display_name: inventory_provider
                        .map(|item| item.display_name.clone())
                        .unwrap_or_else(|| id.clone()),
                    root_path: inventory_provider
                        .and_then(|item| {
                            if id == provider_id {
                                target_root.clone()
                            } else {
                                item.root_path.clone()
                            }
                        })
                        .or_else(|| (id == "agents-directory").then_some(shared_root.clone())),
                    shared: is_shared,
                    reason: is_shared.then(|| {
                        "The selected path is shared with multiple agent consumers".to_string()
                    }),
                }
            })
            .collect::<Vec<_>>();
        let requires_confirmation = impacts.len() > 1 || is_shared;
        let warning = requires_confirmation.then(|| {
            format!(
                "This operation may affect: {}",
                impacts
                    .iter()
                    .map(|impact| impact.provider_id.as_str())
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
            impacts,
            requires_confirmation,
            warning,
        })
    }
}

fn path_is_present(path: &std::path::Path) -> bool {
    path.exists() || path.symlink_metadata().is_ok()
}

fn provider_target_root(
    config: &AppConfig,
    skill: &Skill,
    provider: &SkillProvider,
) -> Option<PathBuf> {
    if skill.scope == SkillScope::Project
        && provider.provider_id != "agents-directory"
        && provider.provider_id != "orca"
    {
        if let Some(project) = skill.project_id.as_deref().and_then(|project_id| {
            config
                .projects
                .iter()
                .find(|project| project.id == project_id)
        }) {
            if let Some(path) =
                WorkspaceService::project_tool_skills_dir(project, &provider.provider_id)
            {
                return Some(path);
            }
        }
    }

    provider.root_path.clone()
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
    let direct = skill.scope == SkillScope::Tool;
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
            .unwrap_or(false);
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
    } else if !provider.detected || provider.reachable == Some(false) {
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
        .filter(|skill| skill.path.strip_prefix(&shared_root).is_ok())
        .collect::<Vec<_>>();
    let shared_consumers = config
        .collect_tool_configs()
        .into_iter()
        .filter(|(_, tool)| tool.skills_path == shared_root)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    if shared_root.exists() || !shared_skills.is_empty() {
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
            root_path: Some(shared_root),
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
    use crate::models::{AppConfig, Skill, SkillScope, SkillSource, ToolConfig};
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
            enabled: HashMap::from([(String::from("claude-code"), false)]),
            path,
        }
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
