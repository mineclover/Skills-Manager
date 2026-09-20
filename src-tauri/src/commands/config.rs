use sm_core::models::AppConfig;
use sm_core::services::ConfigManager;

#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    let manager = ConfigManager::new();
    manager.load()
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    let manager = ConfigManager::new();
    manager.save(&config)
}

#[tauri::command]
pub fn is_initialized() -> bool {
    let manager = ConfigManager::new();
    manager.is_initialized()
}

#[tauri::command]
pub fn mark_initialized() -> Result<(), String> {
    let manager = ConfigManager::new();
    let mut config = manager.load()?;
    config.initialized = true;
    manager.save(&config)?;
    // Best-effort: if Settings → Install CLI already copied the companion
    // skill into the hub during the welcome wizard, enable it now that tools
    // are detected. No-op when the user never installed `skm`.
    let _ = sm_core::services::enable_cli_companion_skill_if_present();
    Ok(())
}
