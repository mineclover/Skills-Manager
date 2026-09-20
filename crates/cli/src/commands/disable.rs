
use sm_core::services::apply_skill_tool_enabled;

use crate::commands::{resolve_skill, resolve_tool_id};
use crate::context::load_config_for_write;

#[derive(clap::Args)]
pub struct Args {
    /// Skill to disable (instance_id or unique id prefix)
    pub skill: String,

    /// Tool to disable the skill for (id or unique prefix)
    #[arg(long = "for")]
    pub for_tool: String,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (config, _guard) = load_config_for_write()?;

    let tool_id = resolve_tool_id(&config, &args.for_tool)?;
    let skill = resolve_skill(&config, &args.skill)?;

    // Mirrors the GUI command: remove the link on disk. The enabled map is
    // derived from disk state on every scan, so no config persistence needed.
    apply_skill_tool_enabled(&config, &skill.instance_id, &tool_id, false, None)
        .map_err(|e| anyhow::anyhow!(e))?;

    println!("disabled '{}' for '{}'", skill.id, tool_id);
    Ok(())
}
