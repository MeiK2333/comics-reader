pub mod error;

use error::{AppError, Result};

/// Echo the trimmed name back to the frontend.
///
/// Commands return [`Result`] so a failure crosses the IPC boundary as a
/// `{ code, message, details }` payload; the frontend picks the localized copy
/// from `code` instead of parsing a string.
#[tauri::command]
fn greet(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput {
            field: "name",
            reason: "must not be empty".into(),
        });
    }

    // Return data only; user-facing copy lives in the frontend i18n catalogs.
    Ok(name.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
