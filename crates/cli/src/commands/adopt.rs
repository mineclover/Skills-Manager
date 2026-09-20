use serde_json::json;
use sm_core::services::{LinkerService, ScannerService};

use crate::context::{acquire_write_lock, load_config};

#[derive(clap::Args)]
pub struct Args {
    /// Show what would be adopted without changing anything
    #[arg(long)]
    pub dry_run: bool,

    /// Apply without prompting
    #[arg(long)]
    pub yes: bool,

    /// Output machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

/// A real (non-symlink) skill directory found inside a tool's skills dir.
struct Candidate {
    tool_id: String,
    path: std::path::PathBuf,
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let config = load_config()?;
    let hub_dir = std::path::PathBuf::from(&config.skills_dir);

    // Find real directories sitting directly in each active tool's skills
    // folder. Symlinks are links we (or the user) already manage — skip them.
    let mut candidates: Vec<Candidate> = Vec::new();
    for (tool_id, tool_config) in config.collect_tool_configs() {
        if !tool_config.enabled || !tool_config.detected {
            continue;
        }
        let skills_path = &tool_config.skills_path;
        let Ok(entries) = std::fs::read_dir(skills_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || sm_core::services::is_symlink_or_junction(&path) {
                continue;
            }
            // Hidden dirs are tool-internal state, not skills
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(true)
            {
                continue;
            }
            candidates.push(Candidate {
                tool_id: tool_id.clone(),
                path,
            });
        }
    }

    // Sort by (skill name, tool id) so the candidate list, the confirmation
    // prompt, and — when two tools hold the same skill name — which copy wins
    // the hub are all deterministic. config.tools is a HashMap, so without this
    // the winner would change from run to run.
    candidates.sort_by(|a, b| {
        a.path
            .file_name()
            .cmp(&b.path.file_name())
            .then_with(|| a.tool_id.cmp(&b.tool_id))
    });

    if candidates.is_empty() {
        if args.json {
            println!("{}", json!({ "adopted": [], "skipped": [] }));
        } else {
            println!("no unmanaged skills found in any tool directory. Nothing to adopt.");
        }
        return Ok(());
    }

    if !args.json {
        println!("found {} candidate(s):", candidates.len());
        for c in &candidates {
            println!(
                "  {}  (from {})",
                c.path.display(),
                c.tool_id
            );
        }
    }

    if args.dry_run {
        return Ok(());
    }

    if !args.yes && !args.json {
        println!();
        println!("these directories will be MOVED into {} and replaced with links.", hub_dir.display());
        print!("continue? [y/N] ");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).ok();
        let answer = answer.trim().to_ascii_lowercase();
        if answer != "y" && answer != "yes" {
            println!("aborted. re-run with --yes to skip this prompt.");
            return Ok(());
        }
    }

    // Lock before mutating anything. The scan and the confirmation prompt stay
    // OUTSIDE the lock on purpose: holding it across an interactive prompt would
    // block every other skm process for as long as the user takes to answer.
    //
    // Reading the config before locking is safe here because nothing we act on
    // can change under us: import_to_hub re-reads the config itself (so its hub
    // path is resolved under this lock), and ConfigManager::load() always
    // normalizes skills_dir back to the default, so the hub cannot move. The
    // pre-lock read only supplies the candidate list the user just confirmed.
    let _guard = acquire_write_lock()?;

    let mut adopted: Vec<String> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();

    for candidate in &candidates {
        let name = candidate
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        // import_to_hub returns Ok(()) *without doing anything* when the hub
        // already holds this name — no move, no symlink. Counting that as
        // "adopted" would tell the user their tool directory was relinked when
        // it is still an unmanaged copy that shows up again on the next run.
        // Merging the two is not safe either: the contents may differ, so leave
        // it in place and say so.
        if name.is_empty() || hub_dir.join(&name).exists() {
            skipped.push((
                candidate.path.to_string_lossy().into_owned(),
                format!("'{}' is already in the hub — left in place", name),
            ));
            continue;
        }

        match LinkerService::import_to_hub(&candidate.path.to_string_lossy()) {
            Ok(()) => adopted.push(name),
            Err(message) => skipped.push((
                candidate.path.to_string_lossy().into_owned(),
                message,
            )),
        }
    }

    if args.json {
        println!(
            "{}",
            json!({
                "adopted": adopted,
                "skipped": skipped.iter().map(|(p, m)| json!({
                    "path": p, "reason": m,
                })).collect::<Vec<_>>(),
            })
        );
        return Ok(());
    }

    if adopted.is_empty() && skipped.is_empty() {
        println!("nothing to do.");
        return Ok(());
    }

    for name in &adopted {
        println!("adopted: {}", name);
    }
    for (path, reason) in &skipped {
        println!("skipped: {} ({})", path, reason);
    }

    let _ = ScannerService::scan_global_skills(&config).map(|skills| {
        println!();
        println!("hub now manages {} skill(s). Run 'skm list' to see them.", skills.len());
    });

    Ok(())
}
