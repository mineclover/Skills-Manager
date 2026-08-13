mod commands;
pub mod models;
pub mod services;
#[cfg(test)]
mod test_support;

use commands::{
    apply_preset, apply_preset_for_scope, apply_preset_for_target, apply_skill_set_activation,
    assign_skill_set_release, batch_set_skill_tools, capture_preset,
    check_marketplace_updates_if_stale, check_sync_status, check_update, clear_active_preset,
    clear_llm_provider, clear_risk_cache_command, clear_translation_cache, clear_usage_stats,
    create_custom_tool, create_directory, create_file, create_preset, create_skill,
    create_skill_set_blueprint, create_skill_set_release, delete_custom_tool, delete_path,
    delete_preset, delete_skill, delete_skill_set_assignment, delete_skill_set_blueprint,
    detect_available_editors, detect_tools, disable_skill, enable_skill, exchange_github_auth,
    exchange_google_auth, export_skills, fetch_clawhub_skill_files,
    fetch_marketplace_skill_descriptions, fetch_marketplace_skills, fetch_skill_file_content,
    fetch_skill_files, fix_sync_issues, get_auth_profile, get_available_editors,
    get_cached_marketplace_translations, get_cached_skill_translations,
    get_cached_text_translation, get_clawhub_categories, get_config, get_contextual_release_health,
    get_llm_provider, get_marketplace_sources, get_release_evaluation_summary, get_release_health,
    get_release_improvement_suggestions, get_risk_report, get_risk_reports_batch,
    get_risk_scanner_version, get_skill_set_catalog, get_skill_set_drift, get_skill_usage_stats,
    get_studio_review_queue, get_tool_status, get_usage_hook_status, import_skills,
    import_skills_to_hub, install_marketplace_skill, install_marketplace_skill_by_ref,
    install_skill_package_from_path, install_usage_hook, is_initialized, list_activation_runs,
    list_marketplace_favorites, list_release_evaluations, list_skill_bindings, list_skill_packages,
    list_skill_providers, list_skills, logout_auth, mark_initialized, open_in_editor,
    preview_clawhub_publish, preview_import_skills, preview_project_binding,
    preview_skill_operation, preview_skill_set_activation, publish_skill_to_clawhub,
    read_directory_tree, read_file, record_release_evaluation, record_studio_feedback,
    refresh_editors, refresh_skills, refresh_tools, register_project_binding,
    remove_project_binding, remove_skill_package, rename_path, rescan_skill,
    resolve_effective_skill_set, review_skill_set_blueprint, save_config, save_llm_provider,
    scan_all_risks, scan_existing_skills, scan_skills_for_scope, set_active_project_binding,
    set_preset_all, set_preset_skill, set_skill_set_assignment_active,
    set_skill_set_assignment_priority, set_tool_enabled, start_github_auth, start_google_auth,
    submit_feedback, sync_marketplace_installed_skills, test_llm_provider,
    toggle_marketplace_favorite, toggle_marketplace_source, toggle_skill_favorite,
    translate_marketplace_skill, translate_skill, translate_skill_files, translate_skills_batch,
    translate_text_content, uninstall_usage_hook, update_custom_tool, update_skill_set_blueprint,
    update_tool_paths, verify_clawhub_token, write_file,
};
use services::{AppCache, MarketplaceCache};
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, mut argv, _cwd| {
            if matches!(argv.first(), Some(arg) if arg.contains("://")) {
                argv.insert(0, String::new());
            }
            let _ = app.emit("auth:deep-link-argv", argv.clone());
            app.deep_link().handle_cli_arguments(argv.into_iter());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                match app.deep_link().register_all() {
                    Ok(_) => {}
                    Err(_err) => {}
                }
                for scheme in ["skills-manager", "skillsmanager"] {
                    match app.deep_link().is_registered(scheme) {
                        Ok(_is_registered) => {}
                        Err(_err) => {}
                    }
                }
            }
            // Sync usage hook state with config on startup
            commands::sync_usage_hook_with_config();
            // Start watching for usage events and emit updates to frontend
            commands::usage::start_usage_watcher(app.handle().clone());
            // Start background risk scan for installed skills
            commands::start_background_scan(app.handle().clone());
            Ok(())
        })
        .manage(AppCache::default())
        .manage(MarketplaceCache::default())
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            is_initialized,
            mark_initialized,
            list_skills,
            refresh_skills,
            list_skill_packages,
            get_skill_set_catalog,
            resolve_effective_skill_set,
            get_skill_set_drift,
            create_skill_set_blueprint,
            review_skill_set_blueprint,
            update_skill_set_blueprint,
            delete_skill_set_blueprint,
            create_skill_set_release,
            assign_skill_set_release,
            set_skill_set_assignment_active,
            set_skill_set_assignment_priority,
            delete_skill_set_assignment,
            preview_skill_set_activation,
            apply_skill_set_activation,
            record_studio_feedback,
            record_release_evaluation,
            get_contextual_release_health,
            get_release_health,
            get_release_evaluation_summary,
            get_release_improvement_suggestions,
            get_studio_review_queue,
            list_activation_runs,
            list_release_evaluations,
            list_skill_providers,
            list_skill_bindings,
            preview_skill_operation,
            preview_project_binding,
            register_project_binding,
            set_active_project_binding,
            remove_project_binding,
            enable_skill,
            disable_skill,
            batch_set_skill_tools,
            apply_preset,
            apply_preset_for_scope,
            apply_preset_for_target,
            capture_preset,
            create_preset,
            delete_preset,
            set_preset_all,
            set_preset_skill,
            scan_skills_for_scope,
            clear_active_preset,
            delete_skill,
            create_skill,
            install_skill_package_from_path,
            remove_skill_package,
            detect_tools,
            refresh_tools,
            get_tool_status,
            set_tool_enabled,
            update_tool_paths,
            create_custom_tool,
            update_custom_tool,
            delete_custom_tool,
            check_sync_status,
            fix_sync_issues,
            scan_existing_skills,
            import_skills_to_hub,
            export_skills,
            preview_import_skills,
            import_skills,
            get_clawhub_categories,
            verify_clawhub_token,
            preview_clawhub_publish,
            publish_skill_to_clawhub,
            detect_available_editors,
            refresh_editors,
            get_available_editors,
            open_in_editor,
            read_directory_tree,
            read_file,
            write_file,
            create_file,
            create_directory,
            delete_path,
            rename_path,
            fetch_marketplace_skills,
            fetch_marketplace_skill_descriptions,
            fetch_skill_files,
            fetch_clawhub_skill_files,
            fetch_skill_file_content,
            install_marketplace_skill,
            install_marketplace_skill_by_ref,
            sync_marketplace_installed_skills,
            check_marketplace_updates_if_stale,
            get_marketplace_sources,
            toggle_marketplace_source,
            toggle_skill_favorite,
            toggle_marketplace_favorite,
            list_marketplace_favorites,
            check_update,
            submit_feedback,
            get_llm_provider,
            save_llm_provider,
            clear_llm_provider,
            test_llm_provider,
            translate_skill,
            translate_skill_files,
            translate_marketplace_skill,
            translate_skills_batch,
            translate_text_content,
            clear_translation_cache,
            get_cached_skill_translations,
            get_cached_marketplace_translations,
            get_cached_text_translation,
            start_github_auth,
            start_google_auth,
            exchange_github_auth,
            exchange_google_auth,
            get_auth_profile,
            logout_auth,
            get_skill_usage_stats,
            install_usage_hook,
            uninstall_usage_hook,
            get_usage_hook_status,
            clear_usage_stats,
            get_risk_report,
            get_risk_reports_batch,
            get_risk_scanner_version,
            scan_all_risks,
            rescan_skill,
            clear_risk_cache_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
