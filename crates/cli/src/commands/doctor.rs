
use serde_json::json;
use sm_core::services::{
    cli_companion_skill_freshness, collect_active_tool_configs, resolve_sync_status,
    CliSkillFreshness, SyncReport, CLI_SKILL_ID,
};

use crate::context::load_config;

#[derive(clap::Args)]
pub struct Args {
    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

struct ToolRow {
    id: String,
    name: String,
    detected: bool,
    enabled: bool,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let config = load_config()?;
    let manager = sm_core::services::ConfigManager::new();

    // Tool detection table
    let tool_rows: Vec<ToolRow> = config
        .collect_tool_configs()
        .into_iter()
        .map(|(id, tool_config)| {
            let name = sm_core::models::tool::SUPPORTED_TOOLS
                .iter()
                .find(|def| def.id == id)
                .map(|def| def.name.to_string())
                .unwrap_or_else(|| {
                    config
                        .custom_tools
                        .get(&id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| id.clone())
                });
            ToolRow {
                id,
                name,
                detected: tool_config.detected,
                enabled: tool_config.enabled,
            }
        })
        .collect();

    // Link issues (same logic as GUI sync status)
    let report: SyncReport = sm_core::services::check_sync_status(&config)
        .map_err(|e| anyhow::anyhow!(e))?;

    // Detail every issue for the human/JSON output
    let skills = sm_core::services::ScannerService::scan_scoped_skills(&config)
        .map_err(|e| anyhow::anyhow!(e))?;
    let mut issues: Vec<serde_json::Value> = Vec::new();
    for (tool_id, tool_config) in collect_active_tool_configs(&config) {
        for skill in &skills {
            let should_be_enabled = skill.is_enabled_for(&tool_id);
            let status = resolve_sync_status(skill, &tool_id, &tool_config);
            if !sm_core::services::should_report_sync_issue(should_be_enabled, status.clone()) {
                continue;
            }
            issues.push(json!({
                "skill": skill.instance_id,
                "skill_id": skill.id,
                "tool": tool_id,
                "expected": if should_be_enabled { "enabled" } else { "disabled" },
                "status": format!("{status:?}"),
            }));
        }
    }

    // An update swaps the binary but nothing rewrites the hub copy of the
    // companion skill, so agents can end up reading instructions for an older
    // `skm`. Report it here rather than silently refreshing: doctor diagnoses,
    // fix repairs.
    let companion = cli_companion_skill_freshness();

    if args.json {
        println!(
            "{}",
            json!({
                "config_initialized": manager.is_initialized(),
                "tools": tool_rows.iter().map(|t| json!({
                    "id": t.id,
                    "name": t.name,
                    "detected": t.detected,
                    "enabled": t.enabled,
                })).collect::<Vec<_>>(),
                "companion_skill": companion.as_str(),
                "issues_count": report.issues_count,
                "issues": issues,
            })
        );
        return Ok(());
    }

    println!("Tools:");
    for tool in &tool_rows {
        let state = match (tool.detected, tool.enabled) {
            (true, true) => "ready",
            (true, false) => "disabled",
            (false, _) => "not installed",
        };
        println!("  {:<20} {:<28} {}", tool.id, tool.name, state);
    }

    println!();
    if report.issues_count == 0 {
        println!("No sync issues found.");
    } else {
        println!("{} sync issue(s):", report.issues_count);
        for issue in &issues {
            println!(
                "  {} @ {}: expected {}, found {}",
                issue["skill_id"].as_str().unwrap_or("?"),
                issue["tool"].as_str().unwrap_or("?"),
                issue["expected"].as_str().unwrap_or("?"),
                issue["status"].as_str().unwrap_or("?"),
            );
        }
        println!();
        println!("Run 'skm fix --yes' to repair them automatically.");
    }

    if companion == CliSkillFreshness::Stale {
        println!();
        println!(
            "Companion skill '{CLI_SKILL_ID}' in the hub does not match this binary\n  \
             (likely an app or CLI update). Run 'skm init' to refresh it."
        );
    }

    Ok(())
}
