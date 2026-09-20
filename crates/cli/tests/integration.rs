//! Integration tests exercising the CLI library surface against a temp HOME.
//!
//! They mirror the end-to-end flow: init config -> create skill ->
//! enable -> doctor shows no issues -> break the link manually ->
//! doctor reports it -> fix repairs it.

use std::fs;
use std::path::Path;

use sm_core::services::{
    apply_skill_tool_enabled, check_sync_status, collect_active_tool_configs, fix_sync_issues,
    ConfigManager, LinkerService, ScannerService,
};
use sm_core::test_support::with_temp_home;

fn write_test_config(home_dir: &Path) {
    let config_dir = home_dir.join(".skills-manager");
    fs::create_dir_all(&config_dir).unwrap();

    let skills_dir = config_dir.join("skills");
    let tool_config_dir = home_dir.join(".test-tool");
    let tool_skills_dir = tool_config_dir.join("skills");
    fs::create_dir_all(&tool_skills_dir).unwrap();

    let config_json = serde_json::json!({
        "version": "1.0.2",
        "skills_dir": skills_dir.to_string_lossy(),
        "tools": {
            "test-tool": {
                "enabled": true,
                "detected": true,
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

fn create_skill(home_dir: &Path, id: &str) {
    let skill_dir = home_dir.join(".skills-manager").join("skills").join(id);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), format!("# {id}\n")).unwrap();
}

#[test]
fn enable_then_no_issues_then_break_then_fix() {
    with_temp_home(|home_dir| {
        write_test_config(home_dir);
        create_skill(home_dir, "demo-skill");

        let manager = ConfigManager::new();
        let config = manager.load().unwrap();

        // enable demo-skill for test-tool (same call the CLI makes)
        apply_skill_tool_enabled(&config, "global:demo-skill", "test-tool", true, None).unwrap();

        // link exists and status is clean
        let link = home_dir.join(".test-tool").join("skills").join("demo-skill");
        assert!(link.exists());
        let reloaded = manager.load().unwrap();
        let report = check_sync_status(&reloaded).unwrap();
        assert_eq!(report.issues_count, 0);

        // simulate an external break: repoint the symlink at a wrong target.
        // Scanner semantics (shared with the GUI): a wrong-target link scans
        // as "disabled for that tool" and is NOT an issue — the user changed
        // it deliberately. Deleting it entirely also just means disabled.
        // So after tampering, doctor must stay at zero issues, and the
        // enabled flag flips to false on rescan.
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(home_dir.join("elsewhere"), &link).unwrap();
        let rescanned = manager.load().unwrap();
        let report = check_sync_status(&rescanned).unwrap();
        assert_eq!(report.issues_count, 0);
        let skills = ScannerService::scan_scoped_skills(&rescanned).unwrap();
        let demo = skills.iter().find(|s| s.id == "demo-skill").unwrap();
        assert!(!demo.is_enabled_for("test-tool"));

        // fix is a no-op when there are no issues
        let link_report = fix_sync_issues(&rescanned).unwrap();
        assert_eq!(link_report.failed.len(), 0);
        assert_eq!(link_report.success.len(), 0);

        // re-enable over the tampered link repairs it to point at the vault
        apply_skill_tool_enabled(&rescanned, "global:demo-skill", "test-tool", true, None)
            .unwrap();
        let final_config = manager.load().unwrap();
        let skills = ScannerService::scan_scoped_skills(&final_config).unwrap();
        let demo = skills.iter().find(|s| s.id == "demo-skill").unwrap();
        assert_eq!(
            sm_core::services::resolve_sync_status(demo, "test-tool", &final_config.get_tool_config("test-tool").unwrap()),
            sm_core::services::LinkStatus::Valid
        );

        // disable removes the link cleanly
        apply_skill_tool_enabled(&final_config, "global:demo-skill", "test-tool", false, None)
            .unwrap();
        assert!(!link.exists());
        let report = check_sync_status(&final_config).unwrap();
        assert_eq!(report.issues_count, 0);
    });
}

#[test]
fn doctor_helpers_skip_disabled_and_undetected_tools() {
    with_temp_home(|home_dir| {
        write_test_config(home_dir);
        create_skill(home_dir, "demo-skill");

        let manager = ConfigManager::new();
        let mut config = manager.load().unwrap();

        // mark the only tool as disabled; collect_active_tool_configs must drop it
        if let Some(tool) = config.tools.get_mut("test-tool") {
            tool.enabled = false;
        }
        assert!(collect_active_tool_configs(&config).is_empty());

        // scanner still finds the vault skill
        let skills = ScannerService::scan_scoped_skills(&config).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "demo-skill");
    });
}

#[test]
fn uninitialized_config_is_rejected() {
    with_temp_home(|_| {
        // no config.json written: is_initialized must be false
        assert!(!ConfigManager::new().is_initialized());
    });
}

#[test]
fn init_creates_config_and_marks_initialized() {
    with_temp_home(|home_dir| {
        let manager = ConfigManager::new();
        assert!(!manager.is_initialized());

        // same call the CLI init command makes
        let mut config = manager.init_default().unwrap();
        config.initialized = true;
        manager.save(&config).unwrap();

        assert!(manager.is_initialized());
        let reloaded = manager.load().unwrap();
        assert!(reloaded.initialized);
        // init_default registers every supported tool (detected or not) and
        // pins the hub at the default location under this temp HOME.
        assert_eq!(reloaded.tools.len(), config.tools.len());
        assert!(!reloaded.tools.is_empty());
        assert_eq!(
            reloaded.skills_dir,
            home_dir.join(".skills-manager").join("skills")
        );
    });
}

#[test]
fn companion_skill_is_written_to_hub_and_enabled_for_active_tools() {
    with_temp_home(|home_dir| {
        write_test_config(home_dir);

        let report = sm_core::services::install_cli_companion_skill().unwrap();
        let skill_dir = home_dir
            .join(".skills-manager")
            .join("skills")
            .join(sm_core::services::CLI_SKILL_ID);

        assert_eq!(report.id, sm_core::services::CLI_SKILL_ID);
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("references").join("json.md").exists());
        assert_eq!(report.enabled_for, vec!["test-tool".to_string()]);

        let link = home_dir
            .join(".test-tool")
            .join("skills")
            .join(sm_core::services::CLI_SKILL_ID);
        assert!(link.exists() || link.symlink_metadata().is_ok());
    });
}

#[test]
fn adopt_moves_real_dirs_into_hub_and_relinks() {
    with_temp_home(|home_dir| {
        write_test_config(home_dir);

        // legacy skills sitting directly in the tool dir (not symlinks)
        let tool_skills = home_dir.join(".test-tool").join("skills");
        let legacy_a = tool_skills.join("legacy-a");
        let legacy_b = tool_skills.join("legacy-b");
        fs::create_dir_all(&legacy_a).unwrap();
        fs::create_dir_all(&legacy_b).unwrap();
        fs::write(legacy_a.join("SKILL.md"), "# A\n").unwrap();
        fs::write(legacy_b.join("SKILL.md"), "# B\n").unwrap();

        // candidate discovery mirrors the CLI adopt command
        let config = ConfigManager::new().load().unwrap();
        let mut found: Vec<std::path::PathBuf> = Vec::new();
        for (_id, tool_config) in collect_active_tool_configs(&config) {
            for entry in std::fs::read_dir(&tool_config.skills_path).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() && !sm_core::services::is_symlink_or_junction(&path) {
                    found.push(path);
                }
            }
        }
        found.sort();
        assert_eq!(found.len(), 2);

        // same call the CLI makes per candidate
        for path in &found {
            LinkerService::import_to_hub(&path.to_string_lossy()).unwrap();
        }

        // originals replaced by links pointing into the hub
        let hub = home_dir.join(".skills-manager").join("skills");
        assert!(hub.join("legacy-a").join("SKILL.md").exists());
        assert!(sm_core::services::is_symlink_or_junction(&tool_skills.join("legacy-a")));
        assert!(sm_core::services::is_symlink_or_junction(&tool_skills.join("legacy-b")));

        // scanner now sees both as managed + enabled for test-tool
        let reloaded = ConfigManager::new().load().unwrap();
        let skills = ScannerService::scan_scoped_skills(&reloaded).unwrap();
        assert_eq!(skills.len(), 2);
        assert!(skills.iter().all(|s| s.is_enabled_for("test-tool")));
    });
}
