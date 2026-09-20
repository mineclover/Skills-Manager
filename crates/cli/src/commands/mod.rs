pub mod adopt;
pub mod disable;
pub mod doctor;
pub mod enable;
pub mod fix;
pub mod init;
pub mod list;

use sm_core::models::{AppConfig, Skill};

/// Resolve a user-supplied skill reference (instance_id or id prefix) to a skill.
///
/// Exact instance_id match wins; otherwise a unique id prefix is accepted.
/// Ambiguous or missing references produce an error listing candidates.
pub fn resolve_skill(config: &AppConfig, reference: &str) -> Result<Skill, anyhow::Error> {
    let skills = sm_core::services::ScannerService::scan_scoped_skills(config)
        .map_err(|e| anyhow::anyhow!(e))?;

    if let Some(found) = skills.iter().find(|s| s.instance_id == reference) {
        return Ok(found.clone());
    }

    let prefix_matches: Vec<&Skill> = skills
        .iter()
        .filter(|s| s.id.starts_with(reference))
        .collect();

    match prefix_matches.len() {
        1 => Ok(prefix_matches[0].clone()),
        0 => Err(anyhow::anyhow!(
            "skill not found: '{}'\nrun 'skm list' to see available skills",
            reference
        )),
        _ => {
            let ids: Vec<&str> = prefix_matches.iter().map(|s| s.id.as_str()).collect();
            Err(anyhow::anyhow!(
                "ambiguous skill reference '{}', matches multiple skills:\n  {}",
                reference,
                ids.join("\n  ")
            ))
        }
    }
}

/// Resolve a user-supplied tool reference by exact id, then unique prefix.
pub fn resolve_tool_id(config: &AppConfig, reference: &str) -> Result<String, anyhow::Error> {
    let mut ids: Vec<String> = config.collect_tool_configs().into_iter().map(|(id, _)| id).collect();
    ids.sort();

    if ids.iter().any(|id| id == reference) {
        return Ok(reference.to_string());
    }

    let matches: Vec<&String> = ids.iter().filter(|id| id.starts_with(reference)).collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Err(anyhow::anyhow!(
            "tool not found: '{}'\nrun 'skm doctor' to see detected tools",
            reference
        )),
        _ => {
            let joined: Vec<&str> = matches.iter().map(|s| s.as_str()).collect();
            Err(anyhow::anyhow!(
                "ambiguous tool reference '{}', matches multiple tools:\n  {}",
                reference,
                joined.join("\n  ")
            ))
        }
    }
}
