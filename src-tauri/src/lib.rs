pub mod error;
mod image;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(image::HttpClient::default())
        .invoke_handler(tauri::generate_handler![
            image::fetch_image,
            image::search_comics,
            image::get_comic
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
