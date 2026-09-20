
use serde_json::json;
use sm_core::services::{resolve_sync_status, collect_active_tool_configs};

use crate::commands::resolve_tool_id;
use crate::context::load_config;

#[derive(clap::Args)]
pub struct Args {
    /// Only show status for this tool (id or unique prefix)
    #[arg(long)]
    pub tool: Option<String>,

    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

struct Row {
    instance_id: String,
    id: String,
    name: String,
    scope: String,
    enabled_tools: Vec<String>,
    link_issues: usize,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let config = load_config()?;
    let skills = sm_core::services::ScannerService::scan_scoped_skills(&config)
        .map_err(|e| anyhow::anyhow!(e))?;

    let active_tools = collect_active_tool_configs(&config);
    let selected_tools: Vec<(String, sm_core::models::ToolConfig)> = match &args.tool {
        Some(tool_ref) => {
            let tool_id = resolve_tool_id(&config, tool_ref)?;
            active_tools
                .into_iter()
                .filter(|(id, _)| *id == tool_id)
                .collect()
        }
        None => active_tools,
    };

    let rows: Vec<Row> = skills
        .iter()
        .map(|skill| {
            let enabled_tools: Vec<String> = selected_tools
                .iter()
                .filter(|(tool_id, _)| skill.is_enabled_for(tool_id))
                .map(|(tool_id, _)| tool_id.clone())
                .collect();

            let link_issues = selected_tools
                .iter()
                .filter(|(tool_id, tool_config)| {
                    skill.is_enabled_for(tool_id)
                        && resolve_sync_status(skill, tool_id, tool_config)
                            != sm_core::services::LinkStatus::Valid
                })
                .count();

            Row {
                instance_id: skill.instance_id.clone(),
                id: skill.id.clone(),
                name: skill.name.clone(),
                scope: match skill.scope {
                    sm_core::models::SkillScope::Global => "global".to_string(),
                    sm_core::models::SkillScope::Project => "project".to_string(),
                },
                enabled_tools,
                link_issues,
            }
        })
        .collect();

    if args.json {
        println!(
            "{}",
            json!({
                "skills": rows.iter().map(|row| json!({
                    "instance_id": row.instance_id,
                    "id": row.id,
                    "name": row.name,
                    "scope": row.scope,
                    "enabled_for": row.enabled_tools,
                    "link_issues": row.link_issues,
                })).collect::<Vec<_>>()
            })
        );
        return Ok(());
    }

    if rows.is_empty() {
        println!("no skills found in the configured skills directory");
        return Ok(());
    }

    // Simple aligned table
    let id_width = rows.iter().map(|r| r.id.len()).max().unwrap_or(2).max(2);
    let name_width = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);

    println!(
        "{:<id_w$}  {:<name_w$}  {:<7}  ENABLED FOR",
        "ID",
        "NAME",
        "SCOPE",
        id_w = id_width,
        name_w = name_width
    );
    for row in &rows {
        let tools = if row.enabled_tools.is_empty() {
            "-".to_string()
        } else {
            row.enabled_tools.join(", ")
        };
        let marker = if row.link_issues > 0 {
            format!("{} ({} broken)", tools, row.link_issues)
        } else {
            tools
        };
        println!(
            "{:<id_w$}  {:<name_w$}  {:<7}  {}",
            row.id,
            row.name,
            row.scope,
            marker,
            id_w = id_width,
            name_w = name_width
        );
    }

    Ok(())
}
