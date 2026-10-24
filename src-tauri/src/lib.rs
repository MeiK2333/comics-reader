pub mod error;
mod image_cache;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(image_cache::HttpClient::default())
        .invoke_handler(tauri::generate_handler![image_cache::fetch_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
