use serde_json::json;
use sm_core::models::AppConfig;
use sm_core::services::{install_cli_companion_skill, CliSkillInstallReport, ConfigManager};

use crate::context::{acquire_init_lock, config_path};

#[derive(clap::Args)]
pub struct Args {
    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

fn detected_tool_ids(config: &AppConfig) -> Vec<String> {
    config
        .tools
        .iter()
        .filter(|(_, tool)| tool.detected)
        .map(|(id, _)| id.clone())
        .collect()
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let manager = ConfigManager::new();

    // Take the lock before deciding, so a concurrent init can't slip between
    // the check and the write. This is the one command allowed to create
    // config.json, hence the init-specific lock.
    let _guard = acquire_init_lock()?;

    // Re-running init must never rebuild the config: init_default() starts from
    // AppConfig::default(), which would drop custom tools, tags, favorites,
    // notes and project bindings. Bail out in BOTH output modes.
    if manager.is_initialized() {
        let config = manager.load().map_err(|e| anyhow::anyhow!(e))?;
        let companion = install_cli_companion_skill();
        print_init_result(args.json, true, &config, companion);
        return Ok(());
    }

    // is_initialized() is also false when config.json exists but cannot be
    // parsed. Overwriting it would silently discard a recoverable config, so
    // refuse and let the user decide.
    let path = config_path();
    let has_content = std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false);
    if has_content && manager.load().is_err() {
        return Err(anyhow::anyhow!(
            "{} exists but could not be parsed.\nMove or fix it, then re-run 'skm init'.",
            path.display()
        ));
    }

    let mut config = manager.init_default().map_err(|e| anyhow::anyhow!(e))?;
    config.initialized = true;
    manager.save(&config).map_err(|e| anyhow::anyhow!(e))?;

    // Read back what was actually persisted: save() normalizes skills_dir, so
    // reporting the in-memory value could describe a directory that is not
    // the one in use.
    let config = manager.load().map_err(|e| anyhow::anyhow!(e))?;
    let companion = install_cli_companion_skill();
    print_init_result(args.json, false, &config, companion);
    Ok(())
}

fn companion_skill_json(result: &Result<CliSkillInstallReport, String>) -> serde_json::Value {
    match result {
        Ok(report) => json!({
            "id": report.id,
            "path": report.path,
            "enabled_for": report.enabled_for,
            "failed": report.failed.iter().map(|item| json!({
                "tool": item.tool,
                "message": item.message,
            })).collect::<Vec<_>>(),
        }),
        Err(message) => json!({ "error": message }),
    }
}

fn print_init_result(
    json_mode: bool,
    already_initialized: bool,
    config: &AppConfig,
    companion: Result<CliSkillInstallReport, String>,
) {
    let detected = detected_tool_ids(config);

    if json_mode {
        println!(
            "{}",
            json!({
                "already_initialized": already_initialized,
                "skills_dir": config.skills_dir,
                "detected_tools": detected,
                "cli_skill": companion_skill_json(&companion),
            })
        );
        return;
    }

    if already_initialized {
        println!("Skills Manager is already initialized.");
    } else {
        println!(
            "initialized. skills directory: {}",
            config.skills_dir.display()
        );
        if detected.is_empty() {
            println!("no AI tools detected — install one and run 'skm doctor'");
        } else {
            println!(
                "detected {} tool(s): {}",
                detected.len(),
                detected.join(", ")
            );
        }
    }

    match &companion {
        Ok(report) => {
            if report.enabled_for.is_empty() {
                println!(
                    "companion skill '{}' installed at {}",
                    report.id,
                    report.path.display()
                );
            } else {
                println!(
                    "companion skill '{}' installed and enabled for: {}",
                    report.id,
                    report.enabled_for.join(", ")
                );
            }
            for item in &report.failed {
                println!("  skipped {}: {}", item.tool, item.message);
            }
        }
        Err(message) => {
            eprintln!("warning: companion skill not installed: {message}");
        }
    }

    if already_initialized {
        return;
    }

    println!();
    println!("next steps:");
    println!("  skm adopt          # bring existing skills under management");
    println!("  skm list           # see what is managed");
}
