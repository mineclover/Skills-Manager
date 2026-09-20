use std::path::PathBuf;

use tauri::Manager;

/// Name of the bundled skm binary inside the app resources dir.
/// CI packages it as `skm` (unix) / `skm.exe` (windows) via tauri.conf
/// bundle.resources.
const SKM_RESOURCE_PATH: &str = "skm";

/// Install location per platform:
/// - macOS/Linux: `/usr/local/bin/skm` when writable, otherwise the first
///   PATH-listed user bin dir, otherwise `~/.local/bin/skm`.
/// - Windows: copy next to the app exe (its dir is on PATH for MSI installs).
///
/// Whether the chosen dir is actually on PATH is answered by `is_on_path` at the
/// call site, not guessed here — the last fallback may well not be on PATH.
fn install_target() -> Result<PathBuf, String> {
    if cfg!(windows) {
        let exe_dir = std::env::current_exe()
            .map_err(|e| format!("Failed to locate app executable: {}", e))?
            .parent()
            .ok_or("Invalid executable path")?
            .to_path_buf();
        return Ok(exe_dir.join("skm.exe"));
    }

    let system_dir = std::path::Path::new("/usr/local/bin");
    if dir_is_writable(system_dir) {
        return Ok(system_dir.join("skm"));
    }

    // Prefer a user bin dir that is already on PATH so the installed binary
    // is immediately usable; ~/.local/bin is the last resort (may need PATH
    // setup, surfaced to the UI via onPath=false).
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    for candidate in [home.join("bin"), home.join(".local").join("bin")] {
        if path_var_contains(&candidate) {
            return Ok(candidate.join("skm"));
        }
    }
    Ok(home.join(".local").join("bin").join("skm"))
}

fn path_var_contains(dir: &std::path::Path) -> bool {
    path_list_contains(&std::env::var_os("PATH").unwrap_or_default(), dir)
}

/// Whether `path_var` (a PATH-formatted OsString) lists `dir`.
///
/// Split out from the env lookup so it is testable without mutating PATH,
/// which would race with other tests in the same process.
fn path_list_contains(path_var: &std::ffi::OsStr, dir: &std::path::Path) -> bool {
    std::env::split_paths(path_var).any(|p| p == dir)
}

/// Whether we can create files in `dir`.
///
/// Opening a directory for writing always fails (EISDIR on unix), so the only
/// reliable probe is creating and removing a real file. Uses a pid-suffixed
/// name to avoid clashing with a concurrent probe.
fn dir_is_writable(dir: &std::path::Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".skm-write-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[tauri::command]
pub fn get_cli_install_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let bundled = resolve_bundled_skm(&app)?;
    let target = install_target()?;
    let installed = target.exists();
    // Version match matters: a stale skm from an older app release should be
    // refreshable.
    let version_matches = installed.then(|| run_version(&target)).flatten()
        .map(|v| v == app.package_info().version.to_string())
        .unwrap_or(false);

    Ok(serde_json::json!({
        "bundled": bundled.exists(),
        "installed": installed,
        "target": target,
        "versionMatches": version_matches,
        "appVersion": app.package_info().version.to_string(),
        // The UI warns when the install dir is not on PATH, otherwise `skm`
        // would be installed but unreachable from a shell.
        "onPath": is_on_path(&target),
    }))
}

#[tauri::command]
pub fn install_cli_binary(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let source = resolve_bundled_skm(&app)?;
    if !source.exists() {
        return Err(format!(
            "bundled CLI not found at {} — this build was packaged without it",
            source.display()
        ));
    }

    let target = install_target()?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }

    std::fs::copy(&source, &target).map_err(|e| {
        format!(
            "Failed to copy CLI to {}: {} (try running the app with permission to write there, or install manually from the release archive)",
            target.display(),
            e
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to make CLI executable: {}", e))?;
    }

    let on_path = is_on_path(&target);
    let cli_skill = match sm_core::services::install_cli_companion_skill() {
        Ok(report) => serde_json::json!({
            "id": report.id,
            "path": report.path,
            "enabled_for": report.enabled_for,
            "failed": report.failed.iter().map(|item| serde_json::json!({
                "tool": item.tool,
                "message": item.message,
            })).collect::<Vec<_>>(),
        }),
        Err(message) => serde_json::json!({ "error": message }),
    };

    Ok(serde_json::json!({
        "installed": true,
        "target": target,
        "onPath": on_path,
        "cliSkill": cli_skill,
    }))
}

fn resolve_bundled_skm(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        format!("{}.exe", SKM_RESOURCE_PATH)
    } else {
        SKM_RESOURCE_PATH.to_string()
    };
    let resource_dir = app
        .path()
        .resolve(".", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve resource dir: {}", e))?;
    Ok(resolve_bundled_skm_in(&resource_dir, &name))
}

/// Pure path logic, testable without an AppHandle.
/// Production: bundle.resources flattens into the resource dir root.
/// Dev: the resource dir is src-tauri/ itself, and bundle:cli stages the
/// binary under src-tauri/resources/.
fn resolve_bundled_skm_in(resource_dir: &std::path::Path, name: &str) -> PathBuf {
    let prod = resource_dir.join(name);
    if prod.exists() {
        return prod;
    }
    let dev = resource_dir.join("resources").join(name);
    if dev.exists() {
        dev
    } else {
        prod
    }
}

fn run_version(binary: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace().last().map(|s| s.to_string())
}

fn is_on_path(target: &std::path::Path) -> bool {
    match target.parent() {
        Some(parent) => path_var_contains(parent),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[cfg(unix)]
    #[test]
    fn resolve_prefers_resource_root_then_dev_fallback() {
        let tmp = std::env::temp_dir().join(format!("skm-res-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let prod_layout = tmp.join("prod");
        std::fs::create_dir_all(&prod_layout).unwrap();
        std::fs::write(prod_layout.join("skm"), b"binary").unwrap();
        assert_eq!(
            resolve_bundled_skm_in(&prod_layout, "skm"),
            prod_layout.join("skm")
        );

        // dev layout: binary under resources/ subdir
        let dev_layout = tmp.join("dev");
        std::fs::create_dir_all(dev_layout.join("resources")).unwrap();
        std::fs::write(dev_layout.join("resources").join("skm"), b"binary").unwrap();
        assert_eq!(
            resolve_bundled_skm_in(&dev_layout, "skm"),
            dev_layout.join("resources").join("skm")
        );

        // missing: falls back to the prod path so the error message shows it
        let empty = tmp.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert_eq!(resolve_bundled_skm_in(&empty, "skm"), empty.join("skm"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn path_list_contains_matches_whole_entries_only() {
        let sep = if cfg!(windows) { ";" } else { ":" };
        let path_var = ["/usr/bin", "/opt/homebrew/bin", "/Users/me/.local/bin"].join(sep);
        let path_var = std::ffi::OsString::from(path_var);

        assert!(path_list_contains(&path_var, Path::new("/opt/homebrew/bin")));
        assert!(path_list_contains(
            &path_var,
            Path::new("/Users/me/.local/bin")
        ));
        // A prefix or parent of a listed entry is not itself on PATH — this is
        // what keeps `~/.local/bin` from being reported as reachable when only
        // `~/.local` happens to appear.
        assert!(!path_list_contains(&path_var, Path::new("/Users/me/.local")));
        assert!(!path_list_contains(&path_var, Path::new("/opt/homebrew")));
        assert!(!path_list_contains(&path_var, Path::new("/usr")));
    }

    #[test]
    fn path_list_contains_handles_an_empty_path_var() {
        assert!(!path_list_contains(
            &std::ffi::OsString::new(),
            Path::new("/usr/bin")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dir_is_writable_rejects_missing_and_unwritable_dirs() {
        let tmp = std::env::temp_dir().join(format!("skm-writable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // A freshly created temp dir is writable. This is the case the old
        // "open the dir for writing" probe got wrong: opening a directory
        // fails with EISDIR, so every dir looked unwritable.
        assert!(dir_is_writable(&tmp));
        // The probe file must not be left behind.
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);

        assert!(!dir_is_writable(&tmp.join("does-not-exist")));

        // A file is not a directory.
        let file = tmp.join("a-file");
        std::fs::write(&file, b"x").unwrap();
        assert!(!dir_is_writable(&file));

        // 0o500 = r-x: listable but not writable.
        use std::os::unix::fs::PermissionsExt;
        let locked = tmp.join("read-only");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(!dir_is_writable(&locked));

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn is_on_path_is_false_for_a_bare_binary_name() {
        // No parent dir means there is nothing to compare against PATH.
        assert!(!is_on_path(Path::new("skm")));
    }
}
