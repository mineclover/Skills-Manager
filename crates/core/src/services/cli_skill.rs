use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::models::{AppConfig, Skill};
use crate::services::linker::{is_symlink_or_junction, remove_symlink_or_junction};
use crate::services::{apply_skill_tool_enabled, collect_active_tool_configs, ConfigManager};

pub const CLI_SKILL_ID: &str = "skills-manager-cli";

const SKILL_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/skills-manager-cli/SKILL.md"
));
const JSON_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/skills-manager-cli/references/json.md"
));
const TOOLS_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../skills/skills-manager-cli/references/tools.md"
));

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliSkillEnableFailure {
    pub tool: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliSkillInstallReport {
    pub id: String,
    pub path: PathBuf,
    pub enabled_for: Vec<String>,
    pub failed: Vec<CliSkillEnableFailure>,
}

/// Copy the bundled `skills-manager-cli` skill into the hub and enable it for
/// every currently active (detected + enabled) tool.
///
/// This is the companion skill agents need in order to drive `skm`. It is
/// product-owned: shipped files are refreshed on every call so a CLI update
/// lands the matching instructions. Extra files the user added under the
/// skill directory are left alone.
///
/// Enable failures for a single tool do not fail the whole install — they
/// accumulate in `failed`. A missing or uninitialized config still writes
/// the hub copy so a later `skm init` / GUI first-run can link it.
pub fn install_cli_companion_skill() -> Result<CliSkillInstallReport, String> {
    let hub = AppConfig::default_skills_dir();
    let dest = materialize_cli_skill(&hub)?;

    let mut report = CliSkillInstallReport {
        id: CLI_SKILL_ID.to_string(),
        path: dest.clone(),
        enabled_for: Vec::new(),
        failed: Vec::new(),
    };

    // Do not call ConfigManager::load() when config.json is missing: load()
    // would init_default() and persist an uninitialized config, racing the
    // GUI welcome wizard. Enable links only once a real init has happened.
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".skills-manager")
        .join("config.json");
    if !config_path.exists() {
        return Ok(report);
    }

    let manager = ConfigManager::new();
    if !manager.is_initialized() {
        return Ok(report);
    }

    let config = manager.load()?;
    enable_for_active_tools(&config, &dest, &mut report);
    Ok(report)
}

/// Enable the companion skill if a previous CLI install already copied it
/// into the hub. Used by the GUI welcome wizard so a CLI installed *before*
/// first-run init still gets linked once tools are detected — without
/// dropping the skill onto machines that never installed `skm`.
pub fn enable_cli_companion_skill_if_present() -> Result<Option<CliSkillInstallReport>, String> {
    let dest = AppConfig::default_skills_dir().join(CLI_SKILL_ID);
    if !dest.join("SKILL.md").exists() && !dest.join("skill.md").exists() {
        return Ok(None);
    }
    install_cli_companion_skill().map(Some)
}

/// Whether the hub copy of the companion skill still matches what this binary
/// ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliSkillFreshness {
    /// Not in the hub: `skm` was never installed, so there is nothing to
    /// refresh.
    NotInstalled,
    /// Hub copy is identical to the files shipped in this binary.
    UpToDate,
    /// A shipped file is missing or its contents drifted. The usual cause is an
    /// app or CLI update that never re-ran the install step.
    Stale,
}

impl CliSkillFreshness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::UpToDate => "up_to_date",
            Self::Stale => "stale",
        }
    }
}

/// Compare the hub copy of the companion skill against the files this binary
/// ships, so `doctor` can report instructions left stale by an update.
///
/// Compares contents rather than a version stamp: the shipped files are the
/// source of truth and `materialize_cli_skill` rewrites them wholesale, so
/// there is no version to read. A user who hand-edits a shipped file therefore
/// reads as `Stale` — correct, since the next install overwrites that edit.
pub fn cli_companion_skill_freshness() -> CliSkillFreshness {
    let dest = AppConfig::default_skills_dir().join(CLI_SKILL_ID);
    if !dest.join("SKILL.md").exists() && !dest.join("skill.md").exists() {
        return CliSkillFreshness::NotInstalled;
    }

    let shipped = [
        (dest.join("SKILL.md"), SKILL_MD),
        (dest.join("references").join("json.md"), JSON_MD),
        (dest.join("references").join("tools.md"), TOOLS_MD),
    ];
    for (path, expected) in shipped {
        match fs::read_to_string(&path) {
            Ok(actual) if actual == expected => {}
            _ => return CliSkillFreshness::Stale,
        }
    }
    CliSkillFreshness::UpToDate
}

fn materialize_cli_skill(hub: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(hub).map_err(|e| format!("Failed to create skills hub: {}", e))?;

    let dest = hub.join(CLI_SKILL_ID);
    if dest.symlink_metadata().is_ok() && is_symlink_or_junction(&dest) {
        remove_symlink_or_junction(&dest)
            .map_err(|e| format!("Failed to replace existing link at {}: {}", dest.display(), e))?;
    }
    if dest.exists() && !dest.is_dir() {
        return Err(format!(
            "Cannot install companion skill: {} exists and is not a directory",
            dest.display()
        ));
    }

    fs::create_dir_all(dest.join("references"))
        .map_err(|e| format!("Failed to create {}: {}", dest.display(), e))?;
    write_shipped_file(&dest.join("SKILL.md"), SKILL_MD)?;
    write_shipped_file(&dest.join("references").join("json.md"), JSON_MD)?;
    write_shipped_file(&dest.join("references").join("tools.md"), TOOLS_MD)?;
    Ok(dest)
}

fn write_shipped_file(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn enable_for_active_tools(
    config: &AppConfig,
    skill_path: &Path,
    report: &mut CliSkillInstallReport,
) {
    let instance_id = Skill::global_instance_id(CLI_SKILL_ID);
    let mut active = collect_active_tool_configs(config);
    active.sort_by(|a, b| a.0.cmp(&b.0));

    for (tool_id, _) in active {
        match apply_skill_tool_enabled(config, &instance_id, &tool_id, true, Some(skill_path)) {
            Ok(()) => report.enabled_for.push(tool_id),
            Err(message) => report.failed.push(CliSkillEnableFailure {
                tool: tool_id,
                message,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_home;
    use std::fs;

    fn write_initialized_config(home_dir: &Path, tool_enabled: bool, tool_detected: bool) {
        let config_dir = home_dir.join(".skills-manager");
        fs::create_dir_all(config_dir.join("skills")).unwrap();

        let tool_config_dir = home_dir.join(".test-tool");
        let tool_skills_dir = tool_config_dir.join("skills");
        fs::create_dir_all(&tool_skills_dir).unwrap();

        let config_json = serde_json::json!({
            "version": "1.0.2",
            "skills_dir": config_dir.join("skills").to_string_lossy(),
            "tools": {
                "test-tool": {
                    "enabled": tool_enabled,
                    "detected": tool_detected,
                    "skills_path": tool_skills_dir.to_string_lossy(),
                    "config_path": tool_config_dir.to_string_lossy()
                }
            },
            "custom_tools": {},
            "initialized": true
        });
        fs::write(
            config_dir.join("config.json"),
            serde_json::to_string_pretty(&config_json).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn bundled_skill_files_are_present_and_named() {
        assert!(SKILL_MD.contains("name: skills-manager-cli"));
        assert!(JSON_MD.contains("skm init --json"));
        assert!(TOOLS_MD.contains("claude-code"));
    }

    #[test]
    fn writes_hub_copy_and_enables_active_tools() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, true, true);

            let report = install_cli_companion_skill().expect("install companion skill");
            let skill_dir = home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID);

            assert_eq!(report.id, CLI_SKILL_ID);
            assert_eq!(report.path, skill_dir);
            assert_eq!(report.enabled_for, vec!["test-tool".to_string()]);
            assert!(report.failed.is_empty());
            assert!(skill_dir.join("SKILL.md").exists());
            assert!(skill_dir.join("references").join("json.md").exists());
            assert!(skill_dir.join("references").join("tools.md").exists());

            let link = home_dir
                .join(".test-tool")
                .join("skills")
                .join(CLI_SKILL_ID);
            assert!(link.exists() || link.symlink_metadata().is_ok());
        });
    }

    #[test]
    fn skips_tools_that_are_disabled_or_undetected() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, false, true);

            let report = install_cli_companion_skill().expect("install companion skill");
            assert!(report.enabled_for.is_empty());
            assert!(report.failed.is_empty());
            assert!(home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("SKILL.md")
                .exists());
            assert!(!home_dir
                .join(".test-tool")
                .join("skills")
                .join(CLI_SKILL_ID)
                .exists());
        });
    }

    #[test]
    fn still_writes_hub_copy_when_config_is_missing() {
        with_temp_home(|home_dir| {
            let report = install_cli_companion_skill().expect("install companion skill");
            assert!(report.enabled_for.is_empty());
            assert!(home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("SKILL.md")
                .exists());
            assert!(
                !home_dir.join(".skills-manager").join("config.json").exists(),
                "must not create config.json before the user runs init"
            );
        });
    }

    #[test]
    fn refreshes_shipped_files_on_a_second_install() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, true, true);
            install_cli_companion_skill().unwrap();

            let skill_md = home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("SKILL.md");
            fs::write(&skill_md, "stale\n").unwrap();

            install_cli_companion_skill().unwrap();
            let contents = fs::read_to_string(&skill_md).unwrap();
            assert!(contents.contains("name: skills-manager-cli"));
            assert!(!contents.starts_with("stale"));
        });
    }

    #[test]
    fn leaves_user_extra_files_in_place() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, true, true);
            install_cli_companion_skill().unwrap();

            let extra = home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("notes.md");
            fs::write(&extra, "keep me\n").unwrap();

            install_cli_companion_skill().unwrap();
            assert_eq!(fs::read_to_string(&extra).unwrap(), "keep me\n");
        });
    }

    #[test]
    fn enable_if_present_is_a_no_op_when_the_skill_was_never_installed() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, true, true);
            let result = enable_cli_companion_skill_if_present().unwrap();
            assert!(result.is_none());
            assert!(!home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .exists());
        });
    }

    #[test]
    fn enable_if_present_links_an_existing_hub_copy() {
        with_temp_home(|home_dir| {
            write_initialized_config(home_dir, true, true);
            install_cli_companion_skill().unwrap();

            let link = home_dir
                .join(".test-tool")
                .join("skills")
                .join(CLI_SKILL_ID);
            let _ = fs::remove_file(&link);
            let _ = fs::remove_dir_all(&link);

            let result = enable_cli_companion_skill_if_present()
                .unwrap()
                .expect("skill is present");
            assert_eq!(result.enabled_for, vec!["test-tool".to_string()]);
            assert!(link.exists() || link.symlink_metadata().is_ok());
        });
    }

    #[test]
    fn freshness_is_not_installed_before_any_install() {
        with_temp_home(|_home_dir| {
            assert_eq!(
                cli_companion_skill_freshness(),
                CliSkillFreshness::NotInstalled
            );
        });
    }

    #[test]
    fn freshness_is_up_to_date_right_after_install() {
        with_temp_home(|_home_dir| {
            install_cli_companion_skill().unwrap();
            assert_eq!(
                cli_companion_skill_freshness(),
                CliSkillFreshness::UpToDate
            );
        });
    }

    #[test]
    fn freshness_is_stale_when_a_shipped_file_drifts() {
        with_temp_home(|home_dir| {
            install_cli_companion_skill().unwrap();
            let json_md = home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("references")
                .join("json.md");
            fs::write(&json_md, "stale\n").unwrap();

            assert_eq!(cli_companion_skill_freshness(), CliSkillFreshness::Stale);
        });
    }

    #[test]
    fn freshness_is_stale_when_a_shipped_file_is_missing() {
        with_temp_home(|home_dir| {
            install_cli_companion_skill().unwrap();
            let tools_md = home_dir
                .join(".skills-manager")
                .join("skills")
                .join(CLI_SKILL_ID)
                .join("references")
                .join("tools.md");
            fs::remove_file(&tools_md).unwrap();

            assert_eq!(cli_companion_skill_freshness(), CliSkillFreshness::Stale);
        });
    }
}
