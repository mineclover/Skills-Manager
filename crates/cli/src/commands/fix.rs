
use serde_json::json;

use crate::context::{load_config, load_config_for_write, ConfigWriteGuard};

#[derive(clap::Args)]
pub struct Args {
    /// Apply fixes without prompting
    #[arg(long)]
    pub yes: bool,

    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    // Without --yes this only reports, so it must not take the write lock and
    // block a concurrent enable/disable. With --yes we lock *before* reading:
    // reading first would let another writer change the config in between, and
    // we would then repair links against a snapshot that no longer exists.
    let (config, _guard): (_, Option<ConfigWriteGuard>) = if args.yes {
        let (config, guard) = load_config_for_write()?;
        (config, Some(guard))
    } else {
        (load_config()?, None)
    };

    let report = sm_core::services::check_sync_status(&config).map_err(|e| anyhow::anyhow!(e))?;

    if report.issues_count == 0 {
        if args.json {
            println!(
                "{}",
                json!({ "fixed": [], "failed": [], "issues_found": 0, "failed_count": 0 })
            );
        } else {
            println!("No sync issues found. Nothing to fix.");
        }
        return Ok(());
    }

    if !args.yes {
        if args.json {
            // Nothing was applied — say so explicitly rather than letting an
            // empty "fixed" list read like a successful no-op run.
            println!(
                "{}",
                json!({
                    "applied": false,
                    "issues_found": report.issues_count,
                    "hint": "re-run with --yes to apply fixes",
                })
            );
            return Ok(());
        }
        println!(
            "{} sync issue(s) found. Re-run with --yes to apply fixes.",
            report.issues_count
        );
        return Ok(());
    }

    // Only symlinks are touched — fix_sync_issues takes &config and never
    // mutates it, so there is nothing to save. Writing the config back here
    // would just clobber concurrent GUI edits with our snapshot.
    let link_report =
        sm_core::services::fix_sync_issues(&config).map_err(|e| anyhow::anyhow!(e))?;

    if args.json {
        println!(
            "{}",
            json!({
                "applied": true,
                "fixed": link_report.success.iter().map(|r| json!({
                    "skill": r.skill_id, "tool": r.tool_id, "message": r.message,
                })).collect::<Vec<_>>(),
                "failed": link_report.failed.iter().map(|r| json!({
                    "skill": r.skill_id, "tool": r.tool_id, "message": r.message,
                })).collect::<Vec<_>>(),
                // Deliberately NOT "issues_count": in doctor's JSON that key
                // means "problems detected", here it would mean "repairs that
                // failed". Same name, different question — so use distinct ones.
                "issues_found": report.issues_count,
                "failed_count": link_report.failed.len(),
            })
        );
        // Fall through to the shared exit-status check below: JSON consumers
        // that only look at the exit code must still see a failed run fail.
    } else {
        for result in &link_report.success {
            println!(
                "fixed {} @ {} ({})",
                result.skill_id,
                result.tool_id,
                result.message.as_deref().unwrap_or("ok")
            );
        }
        for result in &link_report.failed {
            println!(
                "FAILED  {} @ {}: {}",
                result.skill_id,
                result.tool_id,
                result.message.as_deref().unwrap_or("unknown error")
            );
        }
    }

    if !link_report.failed.is_empty() {
        return Err(anyhow::anyhow!(
            "{} fix(es) could not be applied",
            link_report.failed.len()
        ));
    }

    Ok(())
}
