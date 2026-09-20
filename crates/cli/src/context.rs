use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use sm_core::models::AppConfig;
use sm_core::services::ConfigManager;

pub fn config_path() -> PathBuf {
    dirs_home().join(".skills-manager").join("config.json")
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

const NOT_INITIALIZED: &str = "Skills Manager is not initialized yet.\nRun 'skm init' (or launch the Skills Manager desktop app once), then retry.";

pub fn ensure_initialized() -> Result<()> {
    let manager = ConfigManager::new();
    if !manager.is_initialized() {
        return Err(anyhow!(NOT_INITIALIZED));
    }
    Ok(())
}

pub fn load_config() -> Result<AppConfig> {
    ensure_initialized()?;
    ConfigManager::new()
        .load()
        .map_err(|e| anyhow!(e))
        .with_context(|| "failed to load Skills Manager config")
}

/// Guard holding an exclusive advisory lock tied to an open File on
/// config.json. The OS releases the lock when the file is closed (drop).
/// Read-modify-write commands (enable/disable/fix) hold it for the whole run
/// so a concurrent skm process cannot interleave.
///
/// Note: the desktop app does not participate in this lock, so it only
/// serializes skm against skm. GUI writes are still last-writer-wins.
pub struct ConfigWriteGuard {
    _file: File,
}

/// Lock an existing config.json. Commands that reach this all require an
/// initialized config, so the file must already be there.
///
/// Deliberately does NOT create the file: a 0-byte config.json left behind by
/// a lock attempt makes `ConfigManager::load()` take its "exists but
/// unparseable" path forever instead of re-initializing from defaults.
pub fn acquire_write_lock() -> Result<ConfigWriteGuard> {
    lock_config(false)
}

/// Lock for `skm init` — the one command allowed to run before a config
/// exists, and therefore the only one that may create the file.
pub fn acquire_init_lock() -> Result<ConfigWriteGuard> {
    lock_config(true)
}

fn lock_config(create: bool) -> Result<ConfigWriteGuard> {
    let path = config_path();
    if create {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(&path)
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => anyhow!(NOT_INITIALIZED),
            _ => anyhow!("failed to open {} for locking: {}", path.display(), e),
        })?;

    // try_lock_exclusive: fail fast instead of blocking when another process
    // is mid-write.
    fs4::fs_std::FileExt::try_lock_exclusive(&file)
        .map_err(|_| anyhow!("another skm process is modifying the config; try again later"))?;
    Ok(ConfigWriteGuard { _file: file })
}

/// Load config and hold the write lock until the returned guard drops.
///
/// Lock first, then read: reading before locking would let another writer
/// change the file in between, and any subsequent save would clobber it.
pub fn load_config_for_write() -> Result<(AppConfig, ConfigWriteGuard)> {
    let guard = acquire_write_lock()?;
    let config = load_config()?;
    Ok((config, guard))
}
