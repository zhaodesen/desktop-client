mod asr;
mod commands;
mod error;
mod media;
mod model;
mod sidecar;
mod state;
mod storage;
mod store;
mod subtitle;
mod window;

use state::{AppSettings, AppState};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let settings = match store::load_settings(&app_handle) {
                Ok(loaded) => loaded,
                Err(error) => {
                    eprintln!("加载设置失败，回退到默认设置: {error}");
                    let default_settings = AppSettings::default();
                    let _ = store::save_settings(&app_handle, &default_settings);
                    default_settings
                }
            };

            app.manage(AppState::new(settings.clone()));

            window::ensure_overlay_window(&app_handle, &settings).map_err(std::io::Error::other)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::show_overlay,
            commands::hide_overlay,
            commands::toggle_overlay,
            commands::start_asr_job,
            commands::cancel_asr_job,
            commands::get_default_model_status,
            commands::download_default_model,
            commands::get_available_models,
            commands::get_all_models_status,
            commands::get_model_status,
            commands::download_model,
            commands::delete_model,
            commands::get_library_state,
            commands::import_media,
            commands::delete_media,
            commands::update_media_subtitle,
            commands::get_subtitle_document,
            commands::save_subtitle_document,
            commands::translate_media_subtitle,
            commands::record_playback,
            commands::remove_playback_item,
            commands::clear_subtitles,
            commands::clear_audio_cache,
            commands::clear_media_library,
            commands::delete_default_model,
            commands::reset_app_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
