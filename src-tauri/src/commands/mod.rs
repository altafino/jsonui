//! Tauri command layer (integration). Thin: validates ids against the
//! registry, calls `crate::core`, maps errors. No command takes a raw
//! filesystem path from the webview (eng R6).

mod background;
mod document;
mod files;
mod picker;
mod state;

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::Manager;

use crate::core::backup::BackupStore;
use crate::core::presets::PresetRegistry;
use crate::core::registry::Registry;
use state::{AppState, Inner};

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let (config_dir, data_dir) = files::app_dirs(app.handle())?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(&data_dir)?;
    let (registry, notices) = Registry::load(&config_dir);
    let retention = registry.preferences().backup_retention.max(1);
    app.manage(AppState(Mutex::new(Inner {
        registry,
        notices,
        open: HashMap::new(),
        presets: PresetRegistry::bundled(),
        candidates: HashMap::new(),
        backups: BackupStore::new(data_dir.join("backups"), retention),
        data_dir,
    })));
    background::start_poller(app.handle().clone());
    Ok(())
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        files::list_managed_files,
        files::get_startup_notices,
        files::get_file_statuses,
        files::pick_and_add_managed_file,
        files::pick_and_create_managed_file,
        files::create_sample_file,
        files::remove_managed_file,
        files::update_managed_file_metadata,
        files::relocate_managed_file,
        files::confirm_link_write,
        files::list_discoverable_claude_files,
        files::pick_project_directory,
        files::add_discovered_file,
        files::create_missing_file,
        files::pick_schema,
        files::clear_schema,
        files::reveal_managed_file,
        files::open_in_external_editor,
        files::list_leftover_temp_files,
        files::remove_leftover_temp_files,
        files::get_preferences,
        files::set_preferences,
        document::open_document,
        document::reload_document,
        document::get_subtree,
        document::get_field_hints,
        document::get_preset_info,
        document::apply_draft_operation,
        document::undo_draft_operation,
        document::redo_draft_operation,
        document::revert_change,
        document::discard_draft,
        document::preview_draft,
        document::save_draft,
        document::get_conflict,
        document::reapply_draft,
        document::save_draft_as_copy,
        document::list_backups,
        document::preview_restore,
        document::restore_backup,
        document::reveal_masked_value,
    ]
}
