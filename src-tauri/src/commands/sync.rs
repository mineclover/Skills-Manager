use sm_core::services::{
    check_sync_status as core_check_sync_status, fix_sync_issues as core_fix_sync_issues,
    ConfigManager, LinkReport, SyncReport,
};

#[tauri::command]
pub fn check_sync_status() -> Result<SyncReport, String> {
    let config = ConfigManager::new().load()?;
    core_check_sync_status(&config)
}

#[tauri::command]
pub fn fix_sync_issues() -> Result<LinkReport, String> {
    let config = ConfigManager::new().load()?;
    core_fix_sync_issues(&config)
}

#[cfg(test)]
mod tests {
    use sm_core::services::{collect_active_tool_configs, should_report_sync_issue};
    use sm_core::models::{AppConfig, CustomToolConfig, ToolConfig};
    use sm_core::services::LinkStatus;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn mk_tool(enabled: bool, detected: bool) -> ToolConfig {
        ToolConfig {
            enabled,
            detected,
            skills_path: PathBuf::from("/tmp/skills"),
            config_path: PathBuf::from("/tmp/config"),
        }
    }

    #[test]
    fn collect_active_tool_configs_only_returns_enabled_and_detected() {
        let config = AppConfig {
            tools: HashMap::from([
                ("active".to_string(), mk_tool(true, true)),
                ("disabled".to_string(), mk_tool(false, true)),
                ("undetected".to_string(), mk_tool(true, false)),
            ]),
            custom_tools: HashMap::from([(
                "custom-active".to_string(),
                CustomToolConfig {
                    name: "Custom".to_string(),
                    config_path: PathBuf::from("/tmp/custom"),
                    skills_path: PathBuf::from("/tmp/custom/skills"),
                    enabled: true,
                    icon_path: None,
                },
            )]),
            ..AppConfig::default()
        };

        let mut ids: Vec<String> = collect_active_tool_configs(&config)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.sort();

        assert_eq!(ids, vec!["active".to_string()]);
    }

    #[test]
    fn should_report_sync_issue_ignores_wrong_target_for_disabled_skill() {
        assert!(!should_report_sync_issue(false, LinkStatus::WrongTarget));
        // NotALink is external content we must never touch
        assert!(!should_report_sync_issue(false, LinkStatus::NotALink));
    }
}
