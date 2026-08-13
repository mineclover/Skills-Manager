use crate::models::config::SkillActivationPreset;
#[cfg(test)]
use crate::models::AppConfig;
use crate::models::{
    SaveLocalSkillContractRequest, Skill, SkillContractSummary, SkillOperationReport,
};
#[cfg(test)]
use crate::services::skill_control::{
    apply_preset_to_target_with_skills, apply_skill_tool_enabled, build_batch_operations,
    delete_skill_from_disk, rename_tool_skill_for_state, resolve_batch_targets,
    resolve_skill_source_path, BatchSkillToolAction, BatchSkillToolTarget,
    BatchSkillToolTargetKind, ResolvedBatchSkillTarget,
};
#[cfg(test)]
use crate::services::LinkerService;
#[cfg(test)]
use crate::services::ScannerService;
use crate::services::{AppCache, SkillControlService};
use tauri::State;

use crate::services::skill_control::{BatchSetSkillToolsRequest, BatchSetSkillToolsResponse};

#[cfg(test)]
fn load_skill_by_id(config: &AppConfig, skill_id: &str) -> Result<Skill, String> {
    let mut matches = ScannerService::scan_scoped_skills(config)?
        .into_iter()
        .filter(|item| item.id == skill_id)
        .collect::<Vec<_>>();

    if matches.len() > 1 {
        return Err(format!("Ambiguous skill id: {}", skill_id));
    }

    matches
        .pop()
        .ok_or_else(|| format!("Skill not found: {}", skill_id))
}

#[tauri::command]
pub fn batch_set_skill_tools(
    request: BatchSetSkillToolsRequest,
    cache: State<AppCache>,
) -> Result<BatchSetSkillToolsResponse, String> {
    let response = SkillControlService::batch_set_skill_tools(request)?;
    if response.applied_count > 0 {
        cache.invalidate_skills();
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::models::config::{PresetActivation, SkillActivationPreset};
    use crate::models::{InstalledSkillPackage, SkillScope, SkillSource, ToolConfig};
    use crate::services::ConfigManager;
    use crate::test_support::with_temp_home;
    use std::fs;

    use super::*;

    fn create_skill(id: &str, enabled: &[(&str, bool)]) -> Skill {
        Skill {
            id: id.to_string(),
            instance_id: Skill::global_instance_id(id),
            scope: SkillScope::Global,
            project_id: None,
            project_name: None,
            tool_id: None,
            name: id.to_string(),
            description: None,
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            marketplace_meta: None,
            vault_meta: None,
            package_meta: None,
            contract: crate::models::SkillContractSummary::unmanaged(),
            enabled: enabled
                .iter()
                .map(|(tool_id, value)| (tool_id.to_string(), *value))
                .collect(),
            path: PathBuf::from(format!("/tmp/{id}")),
        }
    }

    fn create_nested_skill(id: &str, path: &str, enabled: &[(&str, bool)]) -> Skill {
        Skill {
            id: id.to_string(),
            instance_id: Skill::global_instance_id(id),
            scope: SkillScope::Global,
            project_id: None,
            project_name: None,
            tool_id: None,
            name: id.to_string(),
            description: None,
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            marketplace_meta: None,
            vault_meta: None,
            package_meta: None,
            contract: crate::models::SkillContractSummary::unmanaged(),
            enabled: enabled
                .iter()
                .map(|(tool_id, value)| (tool_id.to_string(), *value))
                .collect(),
            path: PathBuf::from(path),
        }
    }

    fn create_tool_skill(id: &str, tool_id: &str, enabled: bool) -> Skill {
        let mut skill = create_skill(id, &[(tool_id, enabled)]);
        skill.scope = SkillScope::Tool;
        skill.tool_id = Some(tool_id.to_string());
        skill.instance_id = Skill::tool_instance_id(tool_id, id);
        skill
    }

    #[test]
    fn resolve_skill_source_path_uses_skill_path_for_nested_group_member() {
        let config = create_config(&[("claude", true)]);
        let skill = create_nested_skill(
            "baoyu-translate",
            "/tmp/skills/baoyu-skills/baoyu-translate",
            &[("claude", false)],
        );

        assert_eq!(
            resolve_skill_source_path(&config, &skill),
            PathBuf::from("/tmp/skills/baoyu-skills/baoyu-translate")
        );
    }

    #[test]
    fn resolve_skill_source_path_keeps_top_level_skill_path_stable() {
        let config = create_config(&[("claude", true)]);
        let skill = create_skill("plain-skill", &[("claude", false)]);

        assert_eq!(
            resolve_skill_source_path(&config, &skill),
            PathBuf::from("/tmp/plain-skill")
        );
    }

    fn create_package(package_id: &str, installed_members: &[&str]) -> InstalledSkillPackage {
        InstalledSkillPackage {
            package_id: package_id.to_string(),
            name: package_id.to_string(),
            version: "1.0.0".to_string(),
            installed_members: installed_members
                .iter()
                .map(|item| item.to_string())
                .collect(),
            selected_members: installed_members
                .iter()
                .map(|item| item.to_string())
                .collect(),
            path: None,
            manifest_hash: None,
            installed_at: 0,
            updated_at: 0,
        }
    }

    fn create_config(tool_states: &[(&str, bool)]) -> AppConfig {
        let tools = tool_states
            .iter()
            .map(|(tool_id, enabled)| {
                (
                    tool_id.to_string(),
                    ToolConfig {
                        enabled: *enabled,
                        detected: true,
                        skills_path: PathBuf::from(format!("/tmp/{tool_id}/skills")),
                        config_path: PathBuf::from(format!("/tmp/{tool_id}/config")),
                    },
                )
            })
            .collect();

        AppConfig {
            version: "2.0.1".to_string(),
            skills_dir: PathBuf::from("/tmp/skills"),
            tools,
            custom_tools: HashMap::new(),
            skill_metadata: HashMap::new(),
            marketplace_favorites: HashMap::new(),
            preferences: None,
            marketplace_sources: None,
            projects: Vec::new(),
            active_project_id: None,
            llm_provider: None,
            auth_session: None,
            initialized: true,
            presets: Vec::new(),
            active_preset_id: None,
        }
    }

    #[test]
    fn target_preset_controls_managed_and_target_tool_skills() {
        with_temp_home(|home| {
            let global_skills_dir = home.join(".skills-manager").join("skills");
            let tool_skills_dir = home.join(".claude").join("skills");
            fs::create_dir_all(&global_skills_dir).expect("create global skills root");
            fs::create_dir_all(tool_skills_dir.join("direct-existing"))
                .expect("create direct tool skill");
            fs::create_dir_all(tool_skills_dir.join("direct-disabled"))
                .expect("create second direct tool skill");

            for skill_id in ["managed-enabled", "managed-disabled"] {
                let skill_dir = global_skills_dir.join(skill_id);
                fs::create_dir_all(&skill_dir).expect("create managed skill");
                fs::write(
                    skill_dir.join("SKILL.md"),
                    format!("---\nname: {skill_id}\n---\n"),
                )
                .expect("write managed skill");
            }
            fs::write(
                tool_skills_dir.join("direct-existing").join("SKILL.md"),
                "---\nname: direct-existing\n---\n",
            )
            .expect("write direct tool skill");
            fs::write(
                tool_skills_dir.join("direct-disabled").join("SKILL.md"),
                "---\nname: direct-disabled\n---\n",
            )
            .expect("write second direct tool skill");

            let mut config = AppConfig::default();
            config.initialized = true;
            config.skills_dir = global_skills_dir.clone();
            config.tools.insert(
                "claude".to_string(),
                ToolConfig {
                    enabled: true,
                    detected: true,
                    skills_path: tool_skills_dir.clone(),
                    config_path: home.join(".claude"),
                },
            );
            config.presets = vec![SkillActivationPreset {
                id: "preset-target".to_string(),
                name: "Target preset".to_string(),
                description: None,
                activations: vec![PresetActivation {
                    tool_id: "claude".to_string(),
                    skill_ids: vec![
                        "global:managed-enabled".to_string(),
                        "tool:claude:direct-existing".to_string(),
                    ],
                }],
            }];

            let skills =
                ScannerService::scan_skills_for_scope(&config, None).expect("scan preset scope");
            apply_preset_to_target_with_skills("preset-target", "claude", config, skills, None)
                .expect("apply target preset");

            assert!(tool_skills_dir.join("managed-enabled").exists());
            assert!(!tool_skills_dir.join("managed-disabled").exists());
            assert!(tool_skills_dir.join("direct-existing").exists());
            assert!(!tool_skills_dir.join("direct-disabled").exists());
            assert!(tool_skills_dir
                .join("direct-disabled.disabled-by-sm")
                .exists());
        });
    }

    #[test]
    fn preset_control_service_supports_crud_and_membership_updates() {
        with_temp_home(|home| {
            let global_skills_dir = home.join(".skills-manager").join("skills");
            let tool_skills_dir = home.join(".claude").join("skills");
            let skill_dir = global_skills_dir.join("preset-skill");
            fs::create_dir_all(&skill_dir).expect("create global skill");
            fs::write(skill_dir.join("SKILL.md"), "---\nname: preset-skill\n---\n")
                .expect("write global skill");

            let mut config = create_config(&[("claude", true)]);
            config.skills_dir = global_skills_dir;
            let claude_config = config.tools.get_mut("claude").expect("claude config");
            claude_config.skills_path = tool_skills_dir;
            claude_config.config_path = home.join(".claude");
            ConfigManager::new()
                .save(&config)
                .expect("save test config");

            let created = SkillControlService::create_preset(
                "Preset CRUD",
                Some("test preset"),
                false,
                None,
                None,
            )
            .expect("create preset");
            assert_eq!(created.activations.len(), 0);

            let skill_id = Skill::global_instance_id("preset-skill");
            let updated =
                SkillControlService::set_preset_skill(&created.id, None, "claude", &skill_id, true)
                    .expect("enable preset skill");
            assert_eq!(updated.activations[0].skill_ids, vec![skill_id.clone()]);

            let updated = SkillControlService::set_preset_all(&created.id, None, "claude", false)
                .expect("clear preset skills");
            assert!(updated.activations[0].skill_ids.is_empty());

            SkillControlService::delete_preset(&created.id).expect("delete preset");
            let persisted = ConfigManager::new().load().expect("load test config");
            assert!(!persisted
                .presets
                .iter()
                .any(|preset| preset.id == created.id));
        });
    }

    #[test]
    fn rename_tool_skill_for_state_round_trips_enabled_and_disabled_names() {
        with_temp_home(|home| {
            let skills_dir = home.join(".claude").join("skills");
            let enabled_path = skills_dir.join("direct-skill");
            fs::create_dir_all(&enabled_path).expect("create direct skill");

            rename_tool_skill_for_state(&enabled_path, &skills_dir, "direct-skill", false)
                .expect("disable direct skill");
            let disabled_path = skills_dir.join("direct-skill.disabled-by-sm");
            assert!(disabled_path.is_dir());
            assert!(!enabled_path.exists());

            rename_tool_skill_for_state(&disabled_path, &skills_dir, "direct-skill", true)
                .expect("enable direct skill");
            assert!(enabled_path.is_dir());
            assert!(!disabled_path.exists());
        });
    }

    #[test]
    fn rename_tool_skill_for_state_rejects_existing_target_and_outside_paths() {
        with_temp_home(|home| {
            let skills_dir = home.join(".claude").join("skills");
            let enabled_path = skills_dir.join("direct-skill");
            let disabled_path = skills_dir.join("direct-skill.disabled-by-sm");
            fs::create_dir_all(&enabled_path).expect("create enabled skill");
            fs::create_dir_all(&disabled_path).expect("create disabled skill");

            let collision =
                rename_tool_skill_for_state(&enabled_path, &skills_dir, "direct-skill", false)
                    .expect_err("target collision should fail");
            assert!(collision.contains("target already exists"));

            let outside_path = home.join("outside-skill");
            fs::create_dir_all(&outside_path).expect("create outside skill");
            let outside =
                rename_tool_skill_for_state(&outside_path, &skills_dir, "outside-skill", false)
                    .expect_err("outside path should fail");
            assert!(outside.contains("outside the configured skills directory"));
        });
    }

    #[test]
    fn apply_skill_tool_enabled_toggles_direct_tool_skill() {
        with_temp_home(|home| {
            let skills_dir = home.join(".skills-manager").join("skills");
            let tool_skills_dir = home.join(".claude").join("skills");
            let direct_skill_dir = tool_skills_dir.join("direct-skill");
            fs::create_dir_all(&direct_skill_dir).expect("create direct skill");
            fs::write(
                direct_skill_dir.join("SKILL.md"),
                "---\nname: direct-skill\n---\n",
            )
            .expect("write direct skill");

            let mut config = create_config(&[("claude-code", true)]);
            config.skills_dir = skills_dir;
            let claude_config = config
                .tools
                .get_mut("claude-code")
                .expect("claude-code config");
            claude_config.enabled = false;
            claude_config.skills_path = tool_skills_dir.clone();

            apply_skill_tool_enabled(
                &config,
                "tool:claude-code:direct-skill",
                "claude-code",
                false,
                None,
            )
            .expect("disable direct tool skill");
            assert!(tool_skills_dir.join("direct-skill.disabled-by-sm").is_dir());

            apply_skill_tool_enabled(
                &config,
                "tool:claude-code:direct-skill",
                "claude-code",
                true,
                None,
            )
            .expect("enable direct tool skill");
            assert!(direct_skill_dir.is_dir());
        });
    }

    #[test]
    fn apply_skill_tool_enabled_toggles_direct_codex_plugin_state() {
        with_temp_home(|home| {
            let skills_dir = home.join(".skills-manager").join("skills");
            let codex_dir = home.join(".codex");
            let codex_skills_dir = codex_dir.join("skills");
            let direct_skill_dir = codex_skills_dir.join("imagegen");
            fs::create_dir_all(&direct_skill_dir).expect("create codex skill");
            fs::write(
                direct_skill_dir.join("SKILL.md"),
                "---\nname: imagegen\n---\n",
            )
            .expect("write codex skill");

            let mut config = create_config(&[("codex", true)]);
            config.skills_dir = skills_dir;
            let codex_config = config.tools.get_mut("codex").expect("codex config");
            codex_config.enabled = false;
            codex_config.skills_path = codex_skills_dir;
            codex_config.config_path = codex_dir.clone();

            apply_skill_tool_enabled(&config, "tool:codex:imagegen", "codex", false, None)
                .expect("disable codex plugin");
            assert!(fs::read_to_string(codex_dir.join("config.toml"))
                .expect("read codex config")
                .contains("enabled = false"));

            apply_skill_tool_enabled(&config, "tool:codex:imagegen", "codex", true, None)
                .expect("enable codex plugin");
            assert!(fs::read_to_string(codex_dir.join("config.toml"))
                .expect("read codex config")
                .contains("enabled = true"));
        });
    }

    #[test]
    fn apply_skill_tool_enabled_enables_nested_group_member_from_real_skill_path() {
        with_temp_home(|home| {
            let skills_dir = home.join(".skills-manager").join("skills");
            let nested_skill_dir = skills_dir.join("baoyu-skills").join("baoyu-translate");
            fs::create_dir_all(&nested_skill_dir).expect("create nested skill dir");
            fs::write(
                nested_skill_dir.join("SKILL.md"),
                "---\nname: baoyu-translate\n---\n",
            )
            .expect("write SKILL.md");

            let tool_skills_dir = home.join(".claude").join("skills");
            let config = AppConfig {
                version: "2.0.1".to_string(),
                skills_dir: skills_dir.clone(),
                tools: HashMap::from([(
                    "claude".to_string(),
                    ToolConfig {
                        enabled: true,
                        detected: true,
                        skills_path: tool_skills_dir.clone(),
                        config_path: home.join(".claude"),
                    },
                )]),
                custom_tools: HashMap::new(),
                skill_metadata: HashMap::new(),
                marketplace_favorites: HashMap::new(),
                preferences: None,
                marketplace_sources: None,
                projects: Vec::new(),
                active_project_id: None,
                llm_provider: None,
                auth_session: None,
                initialized: true,
                presets: Vec::new(),
                active_preset_id: None,
            };

            apply_skill_tool_enabled(&config, "global:baoyu-translate", "claude", true, None)
                .expect("enable nested group member");

            let link_path = tool_skills_dir.join("baoyu-translate");
            assert!(link_path.exists() || link_path.symlink_metadata().is_ok());
            let target = fs::read_link(&link_path).expect("read created symlink");
            assert_eq!(target, nested_skill_dir);
        });
    }

    #[test]
    fn delete_skill_from_disk_removes_nested_group_member_from_real_path() {
        with_temp_home(|home| {
            let skills_dir = home.join(".skills-manager").join("skills");
            let nested_skill_dir = skills_dir.join("baoyu-skills").join("baoyu-translate");
            fs::create_dir_all(&nested_skill_dir).expect("create nested skill dir");
            fs::write(
                nested_skill_dir.join("SKILL.md"),
                "---\nname: baoyu-translate\n---\n",
            )
            .expect("write SKILL.md");

            let tool_skills_dir = home.join(".claude").join("skills");
            fs::create_dir_all(&tool_skills_dir).expect("create tool skills dir");
            LinkerService::enable_skill_for_tool(
                &nested_skill_dir,
                &tool_skills_dir,
                "baoyu-translate",
                "claude",
            )
            .expect("create tool link");

            let config = AppConfig {
                version: "2.0.1".to_string(),
                skills_dir: skills_dir.clone(),
                tools: HashMap::from([(
                    "claude".to_string(),
                    ToolConfig {
                        enabled: true,
                        detected: true,
                        skills_path: tool_skills_dir.clone(),
                        config_path: home.join(".claude"),
                    },
                )]),
                custom_tools: HashMap::new(),
                skill_metadata: HashMap::new(),
                marketplace_favorites: HashMap::new(),
                preferences: None,
                marketplace_sources: None,
                projects: Vec::new(),
                active_project_id: None,
                llm_provider: None,
                auth_session: None,
                initialized: true,
                presets: Vec::new(),
                active_preset_id: None,
            };

            delete_skill_from_disk(&config, "global:baoyu-translate").expect("delete nested skill");

            assert!(!nested_skill_dir.exists());
            assert!(tool_skills_dir
                .join("baoyu-translate")
                .symlink_metadata()
                .is_err());
        });
    }

    #[test]
    fn resolve_batch_targets_expands_groups_and_reports_missing_members() {
        let skills_by_id = HashMap::from([
            ("skill-a".to_string(), create_skill("skill-a", &[])),
            ("skill-b".to_string(), create_skill("skill-b", &[])),
        ]);
        let packages_by_id = HashMap::from([(
            "group-one".to_string(),
            create_package("group-one", &["skill-a", "missing-skill"]),
        )]);

        let (resolved, failures) = resolve_batch_targets(
            &[BatchSkillToolTarget {
                kind: BatchSkillToolTargetKind::Group,
                id: "group-one".to_string(),
            }],
            &skills_by_id,
            &packages_by_id,
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].skill_id, "global:skill-a");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].skill_id.as_deref(), Some("missing-skill"));
    }

    #[test]
    fn build_batch_operations_deduplicates_overlapping_skill_and_group_targets() {
        let skills_by_id = HashMap::from([(
            "skill-a".to_string(),
            create_skill("skill-a", &[("claude", false)]),
        )]);
        let config = create_config(&[("claude", true)]);
        let resolved_targets = vec![
            ResolvedBatchSkillTarget {
                target_kind: BatchSkillToolTargetKind::Skill,
                target_id: "skill-a".to_string(),
                skill_id: "skill-a".to_string(),
            },
            ResolvedBatchSkillTarget {
                target_kind: BatchSkillToolTargetKind::Group,
                target_id: "group-one".to_string(),
                skill_id: "skill-a".to_string(),
            },
        ];

        let (plan, failures) = build_batch_operations(
            &resolved_targets,
            &["claude".to_string()],
            &skills_by_id,
            &config,
            &BatchSkillToolAction::Enable,
        );

        assert!(failures.is_empty());
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.skipped_count, 0);
    }

    #[test]
    fn build_batch_operations_skips_already_enabled_and_keeps_disabled_tools_actionable() {
        let skills_by_id = HashMap::from([(
            "skill-a".to_string(),
            create_skill("skill-a", &[("claude", true)]),
        )]);
        let config = create_config(&[("claude", true), ("codex", false)]);
        let resolved_targets = vec![ResolvedBatchSkillTarget {
            target_kind: BatchSkillToolTargetKind::Skill,
            target_id: "skill-a".to_string(),
            skill_id: "skill-a".to_string(),
        }];

        let (plan, failures) = build_batch_operations(
            &resolved_targets,
            &["claude".to_string(), "codex".to_string()],
            &skills_by_id,
            &config,
            &BatchSkillToolAction::Enable,
        );

        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.skipped_count, 1);
        assert!(failures.is_empty());
        assert_eq!(plan.operations[0].tool_id, "codex");
    }

    #[test]
    fn build_batch_operations_only_targets_a_tool_skill_owner() {
        let skill = create_tool_skill("direct-skill", "claude", true);
        let skills_by_id = HashMap::from([(skill.instance_id.clone(), skill)]);
        let config = create_config(&[("claude", true), ("codex", true)]);
        let resolved_targets = vec![ResolvedBatchSkillTarget {
            target_kind: BatchSkillToolTargetKind::Skill,
            target_id: "tool:claude:direct-skill".to_string(),
            skill_id: "tool:claude:direct-skill".to_string(),
        }];

        let (plan, failures) = build_batch_operations(
            &resolved_targets,
            &["claude".to_string(), "codex".to_string()],
            &skills_by_id,
            &config,
            &BatchSkillToolAction::Disable,
        );

        assert!(failures.is_empty());
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.operations[0].tool_id, "claude");
        assert_eq!(plan.skipped_count, 1);
    }

    #[test]
    fn build_batch_operations_ignores_duplicate_skips_and_failures_for_overlapping_targets() {
        let skills_by_id = HashMap::from([(
            "skill-a".to_string(),
            create_skill("skill-a", &[("claude", true)]),
        )]);
        let config = create_config(&[("claude", true), ("codex", false)]);
        let resolved_targets = vec![
            ResolvedBatchSkillTarget {
                target_kind: BatchSkillToolTargetKind::Skill,
                target_id: "skill-a".to_string(),
                skill_id: "skill-a".to_string(),
            },
            ResolvedBatchSkillTarget {
                target_kind: BatchSkillToolTargetKind::Group,
                target_id: "group-one".to_string(),
                skill_id: "skill-a".to_string(),
            },
        ];

        let (plan, failures) = build_batch_operations(
            &resolved_targets,
            &["claude".to_string(), "codex".to_string()],
            &skills_by_id,
            &config,
            &BatchSkillToolAction::Enable,
        );

        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.skipped_count, 1);
        assert!(failures.is_empty());
        assert_eq!(plan.operations[0].tool_id, "codex");
    }

    #[test]
    fn resolve_batch_targets_uses_instance_ids_for_skills() {
        let global_skill = create_skill("shared-skill", &[]);
        let project_skill = create_skill("shared-skill", &[]).with_scope(
            SkillScope::Project,
            Some("project-alpha".to_string()),
            Some("Project Alpha".to_string()),
        );
        let skills_by_instance_id = HashMap::from([
            (global_skill.instance_id.clone(), global_skill.clone()),
            (project_skill.instance_id.clone(), project_skill.clone()),
        ]);
        let packages_by_id = HashMap::new();

        let (resolved, failures) = resolve_batch_targets(
            &[BatchSkillToolTarget {
                kind: BatchSkillToolTargetKind::Skill,
                id: project_skill.instance_id.clone(),
            }],
            &skills_by_instance_id,
            &packages_by_id,
        );

        assert!(failures.is_empty());
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].skill_id, project_skill.instance_id);
    }

    #[test]
    fn resolve_batch_targets_prefers_global_instance_for_group_members() {
        let global_skill = create_skill("shared-skill", &[]);
        let project_skill = create_skill("shared-skill", &[]).with_scope(
            SkillScope::Project,
            Some("project-alpha".to_string()),
            Some("Project Alpha".to_string()),
        );
        let skills_by_instance_id = HashMap::from([
            (global_skill.instance_id.clone(), global_skill.clone()),
            (project_skill.instance_id.clone(), project_skill.clone()),
        ]);
        let packages_by_id = HashMap::from([(
            "group-one".to_string(),
            create_package("group-one", &["shared-skill"]),
        )]);

        let (resolved, failures) = resolve_batch_targets(
            &[BatchSkillToolTarget {
                kind: BatchSkillToolTargetKind::Group,
                id: "group-one".to_string(),
            }],
            &skills_by_instance_id,
            &packages_by_id,
        );

        assert!(failures.is_empty());
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].skill_id, global_skill.instance_id);
    }

    #[test]
    fn load_skill_by_id_rejects_ambiguous_legacy_skill_ids() {
        with_temp_home(|home| {
            let global_skills_dir = home.join(".skills-manager").join("skills");
            let project_root = home.join("code").join("project-alpha");
            let project_skills_dir = project_root.join(".claude").join("skills");
            fs::create_dir_all(global_skills_dir.join("shared-skill"))
                .expect("create global shared skill");
            fs::create_dir_all(project_skills_dir.join("shared-skill"))
                .expect("create project shared skill");
            fs::write(
                global_skills_dir.join("shared-skill").join("SKILL.md"),
                "---\nname: shared-skill\n---\n",
            )
            .expect("write global skill");
            fs::write(
                project_skills_dir.join("shared-skill").join("SKILL.md"),
                "---\nname: shared-skill\n---\n",
            )
            .expect("write project skill");

            let config: AppConfig = serde_json::from_value(serde_json::json!({
                "version": "2.0.1",
                "skills_dir": global_skills_dir,
                "tools": {},
                "custom_tools": {},
                "skill_metadata": {},
                "preferences": null,
                "marketplace_sources": null,
                "projects": [{
                    "id": "project-alpha",
                    "name": "Project Alpha",
                    "root_path": project_root,
                    "skills_dir": project_skills_dir,
                }],
                "active_project_id": "project-alpha",
                "initialized": true,
            }))
            .expect("deserialize config");

            let error = load_skill_by_id(&config, "shared-skill")
                .expect_err("legacy skill id should be ambiguous");

            assert!(error.contains("Ambiguous skill id: shared-skill"));
        });
    }
}

#[tauri::command]
pub fn list_skills(cache: State<AppCache>) -> Result<Vec<Skill>, String> {
    // Try to get from cache first
    if let Some(skills) = cache.get_skills() {
        return Ok(skills);
    }

    // Cache miss - scan and cache
    let skills = SkillControlService::list_scoped_skills()?;
    cache.set_skills(skills.clone());
    Ok(skills)
}

#[tauri::command]
pub fn enable_skill(
    instance_id: String,
    tool_id: String,
    cache: State<AppCache>,
) -> Result<SkillOperationReport, String> {
    let report = SkillControlService::set_skill_enabled(&instance_id, &tool_id, true)?;

    // Invalidate cache after modification
    cache.invalidate_skills();
    Ok(report)
}

#[tauri::command]
pub fn disable_skill(
    instance_id: String,
    tool_id: String,
    cache: State<AppCache>,
) -> Result<SkillOperationReport, String> {
    let report = SkillControlService::set_skill_enabled(&instance_id, &tool_id, false)?;

    // Invalidate cache after modification
    cache.invalidate_skills();
    Ok(report)
}

#[tauri::command]
pub fn scan_existing_skills() -> Result<Vec<crate::models::Skill>, String> {
    crate::services::ScannerService::scan_all_tools()
}

#[tauri::command]
pub fn import_skills_to_hub(
    skill_paths: Vec<String>,
    cache: State<AppCache>,
) -> Result<(), String> {
    SkillControlService::import_skills_to_hub(&skill_paths)?;
    // Invalidate cache after import
    cache.invalidate_skills();
    Ok(())
}

#[tauri::command]
pub fn delete_skill(instance_id: String, cache: State<AppCache>) -> Result<(), String> {
    SkillControlService::delete_skill(&instance_id)?;

    // Invalidate cache after deletion
    cache.invalidate_skills();
    Ok(())
}

#[tauri::command]
pub fn create_skill(
    name: String,
    description: Option<String>,
    cache: State<AppCache>,
) -> Result<Skill, String> {
    let skill = SkillControlService::create_skill(&name, description.as_deref())?;

    // Invalidate cache
    cache.invalidate_skills();

    Ok(skill)
}

#[tauri::command]
pub fn save_local_skill_contract(
    request: SaveLocalSkillContractRequest,
    cache: State<AppCache>,
) -> Result<SkillContractSummary, String> {
    let summary = SkillControlService::save_local_skill_contract(request)?;
    cache.invalidate_skills();
    Ok(summary)
}

#[tauri::command]
pub fn refresh_skills(cache: State<AppCache>) -> Result<Vec<Skill>, String> {
    // Refresh is intentionally read-only. Directly installed Tool skills must remain
    // in their owning tool directory; importing into the hub is an explicit action.
    let skills = SkillControlService::list_scoped_skills()?;
    cache.set_skills(skills.clone());
    Ok(skills)
}

#[tauri::command]
pub fn scan_skills_for_scope(project_id: Option<String>) -> Result<Vec<Skill>, String> {
    SkillControlService::list_skills(project_id.as_deref())
}

#[tauri::command]
pub fn apply_preset(
    preset_id: String,
    cache: State<AppCache>,
) -> Result<SkillOperationReport, String> {
    let report = SkillControlService::apply_preset(&preset_id)?;
    cache.invalidate_skills();
    Ok(report)
}

#[tauri::command]
pub fn apply_preset_for_scope(
    preset_id: String,
    project_id: Option<String>,
    cache: State<AppCache>,
) -> Result<SkillOperationReport, String> {
    let report = SkillControlService::apply_preset_for_scope(&preset_id, project_id.as_deref())?;
    cache.invalidate_skills();
    Ok(report)
}

#[tauri::command]
pub fn apply_preset_for_target(
    preset_id: String,
    project_id: Option<String>,
    tool_id: String,
    cache: State<AppCache>,
) -> Result<SkillOperationReport, String> {
    let report =
        SkillControlService::apply_preset_for_target(&preset_id, project_id.as_deref(), &tool_id)?;
    cache.invalidate_skills();
    Ok(report)
}

#[tauri::command]
pub fn clear_active_preset(cache: State<AppCache>) -> Result<(), String> {
    SkillControlService::clear_active_preset()?;
    cache.invalidate_skills();
    Ok(())
}

#[tauri::command]
pub fn create_preset(
    name: String,
    description: Option<String>,
    copy_current_state: bool,
    project_id: Option<String>,
    tool_id: Option<String>,
) -> Result<SkillActivationPreset, String> {
    SkillControlService::create_preset(
        &name,
        description.as_deref(),
        copy_current_state,
        project_id.as_deref(),
        tool_id.as_deref(),
    )
}

#[tauri::command]
pub fn delete_preset(preset_id: String) -> Result<(), String> {
    SkillControlService::delete_preset(&preset_id)
}

#[tauri::command]
pub fn capture_preset(
    preset_id: String,
    project_id: Option<String>,
    tool_id: String,
) -> Result<SkillActivationPreset, String> {
    SkillControlService::capture_preset(&preset_id, project_id.as_deref(), &tool_id)
}

#[tauri::command]
pub fn set_preset_skill(
    preset_id: String,
    project_id: Option<String>,
    tool_id: String,
    skill_id: String,
    enabled: bool,
) -> Result<SkillActivationPreset, String> {
    SkillControlService::set_preset_skill(
        &preset_id,
        project_id.as_deref(),
        &tool_id,
        &skill_id,
        enabled,
    )
}

#[tauri::command]
pub fn set_preset_all(
    preset_id: String,
    project_id: Option<String>,
    tool_id: String,
    enabled: bool,
) -> Result<SkillActivationPreset, String> {
    SkillControlService::set_preset_all(&preset_id, project_id.as_deref(), &tool_id, enabled)
}
