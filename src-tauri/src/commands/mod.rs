//! Tauri command layer (integration). Thin: validates ids against the
//! registry, calls `crate::core`, maps errors. No command takes a raw
//! filesystem path from the webview (eng R6).

pub fn setup(_app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![]
}
