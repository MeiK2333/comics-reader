mod ech;
pub mod error;
mod image;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // rustls has both ring (reqwest/hickory) and aws-lc-rs (ECH HPKE) enabled.
    // Install ring as the process default so existing clients keep their
    // provider; the ECH handshake passes aws-lc-rs explicitly.
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(image::HttpClient::default())
        .invoke_handler(tauri::generate_handler![
            image::fetch_image,
            image::search_comics,
            image::get_comic,
            image::set_http_proxy,
            ech::fetch_ech_config,
            ech::probe_ech_handshake,
            ech::set_ech_enabled,
            ech::get_ech_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
