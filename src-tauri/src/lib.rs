mod downloads;
mod ech;
pub mod error;
mod favorites;
mod history;
mod image;
mod logging;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // rustls has both ring (reqwest/hickory) and aws-lc-rs (ECH HPKE) enabled.
    // Install ring as the process default so existing clients keep their
    // provider; the ECH handshake passes aws-lc-rs explicitly.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Logging is opt-in and must never prevent startup. Commands retry
            // path resolution if this diagnostic setup fails.
            if let Err(error) = logging::init(app.handle()) {
                eprintln!("failed to initialize the application log: {error}");
            }
            Ok(())
        })
        .manage(image::HttpClient::default())
        .manage(history::HistoryStore::default())
        .manage(favorites::FavoritesStore::default())
        .manage(downloads::DownloadsStore::default())
        .invoke_handler(tauri::generate_handler![
            image::fetch_image,
            image::search_comics,
            image::get_comic,
            image::get_reader_chapter,
            image::set_http_proxy,
            history::list_history,
            history::record_history,
            history::get_history_entry,
            favorites::list_favorites,
            favorites::get_favorite,
            favorites::add_favorite,
            favorites::remove_favorite,
            favorites::list_favorite_groups,
            favorites::create_favorite_group,
            favorites::rename_favorite_group,
            favorites::delete_favorite_group,
            downloads::commands::start_download,
            downloads::commands::pause_download,
            downloads::commands::resume_download,
            downloads::commands::delete_download,
            downloads::commands::list_downloads,
            downloads::commands::get_download,
            downloads::commands::get_downloaded_chapter,
            ech::fetch_ech_config,
            ech::probe_ech_handshake,
            ech::set_ech_enabled,
            ech::get_ech_status,
            logging::set_logging_enabled,
            logging::read_logs,
            logging::export_logs,
            logging::clear_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
