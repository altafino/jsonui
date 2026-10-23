//! Native pickers run Rust-side so paths never come from the webview (eng R6).
//!
//! For automated real-app tests, `CONFIGDECK_TEST_PICKS` may point to a file
//! with one path per line; each picker call consumes the first line (an empty
//! line means "cancelled"). Without the variable, native dialogs are used.

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;

pub enum PickKind {
    OpenJson,
    SaveJson { suggested_name: String },
    Folder,
    Schema,
}

pub fn pick<R: Runtime>(app: &AppHandle<R>, kind: PickKind) -> Option<PathBuf> {
    if let Some(path) = test_pick() {
        return path;
    }
    let dialog = app.dialog().file();
    let picked = match kind {
        PickKind::OpenJson => dialog
            .set_title("Add JSON file")
            .add_filter("JSON / JSONC", &["json", "jsonc"])
            .blocking_pick_file(),
        PickKind::SaveJson { suggested_name } => dialog
            .set_title("Save as")
            .set_file_name(suggested_name)
            .add_filter("JSON / JSONC", &["json", "jsonc"])
            .blocking_save_file(),
        PickKind::Folder => dialog
            .set_title("Choose project folder")
            .blocking_pick_folder(),
        PickKind::Schema => dialog
            .set_title("Associate JSON Schema")
            .add_filter("JSON Schema", &["json"])
            .blocking_pick_file(),
    };
    picked.and_then(|p| p.into_path().ok())
}

/// `Some(None)` = scripted cancel, `Some(Some(path))` = scripted pick,
/// `None` = no script configured.
fn test_pick() -> Option<Option<PathBuf>> {
    let script = std::env::var_os("CONFIGDECK_TEST_PICKS")?;
    let content = std::fs::read_to_string(&script).ok()?;
    let mut lines = content.lines();
    let first = lines.next()?.trim().to_string();
    let rest: Vec<&str> = lines.collect();
    let _ = std::fs::write(&script, rest.join("\n"));
    Some(if first.is_empty() {
        None
    } else {
        Some(PathBuf::from(first))
    })
}
