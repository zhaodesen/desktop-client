use crate::{
    asr::{self, StartAsrJobInput, StartAsrJobOutput},
    error::CommandResponse,
    media::{self, ImportMediaInput, LibraryState, MediaItem, PlaybackHistoryItem},
    model::{self, DefaultModelStatus, DownloadModelOutput},
    state::{AppSettings, AppState, OverlayWindowState},
    storage::{self, CleanupResult},
    store, window,
};
use tauri::{AppHandle, State};

fn update_settings_in_state(
    app: &AppHandle,
    state: &State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| "应用设置锁已损坏".to_string())?;
        *guard = settings.clone();
    }

    store::save_settings(app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CommandResponse<AppSettings> {
    match state.settings.lock() {
        Ok(guard) => CommandResponse::ok(guard.clone()),
        Err(_) => CommandResponse::err("settings_lock_failed", "读取设置失败"),
    }
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResponse<AppSettings> {
    let visibility_result = if settings.overlay_visible {
        window::show_overlay(&app)
    } else {
        window::hide_overlay(&app)
    };

    if let Err(error) = visibility_result {
        return CommandResponse::err("overlay_visibility_failed", error);
    }

    match update_settings_in_state(&app, &state, settings) {
        Ok(saved) => CommandResponse::ok(saved),
        Err(error) => CommandResponse::err("settings_save_failed", error),
    }
}

#[tauri::command]
pub fn show_overlay(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<OverlayWindowState> {
    match window::show_overlay(&app) {
        Ok(visible) => {
            let current_settings = match state.settings.lock() {
                Ok(mut guard) => {
                    guard.overlay_visible = visible;
                    guard.clone()
                }
                Err(_) => {
                    return CommandResponse::err("settings_lock_failed", "更新设置失败");
                }
            };

            if let Err(error) = store::save_settings(&app, &current_settings) {
                return CommandResponse::err("settings_save_failed", error);
            }

            CommandResponse::ok(OverlayWindowState { visible })
        }
        Err(error) => CommandResponse::err("overlay_show_failed", error),
    }
}

#[tauri::command]
pub fn hide_overlay(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<OverlayWindowState> {
    match window::hide_overlay(&app) {
        Ok(visible) => {
            let current_settings = match state.settings.lock() {
                Ok(mut guard) => {
                    guard.overlay_visible = visible;
                    guard.clone()
                }
                Err(_) => {
                    return CommandResponse::err("settings_lock_failed", "更新设置失败");
                }
            };

            if let Err(error) = store::save_settings(&app, &current_settings) {
                return CommandResponse::err("settings_save_failed", error);
            }

            CommandResponse::ok(OverlayWindowState { visible })
        }
        Err(error) => CommandResponse::err("overlay_hide_failed", error),
    }
}

#[tauri::command]
pub fn toggle_overlay(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<OverlayWindowState> {
    match window::toggle_overlay(&app) {
        Ok(visible) => {
            let current_settings = match state.settings.lock() {
                Ok(mut guard) => {
                    guard.overlay_visible = visible;
                    guard.clone()
                }
                Err(_) => {
                    return CommandResponse::err("settings_lock_failed", "更新设置失败");
                }
            };

            if let Err(error) = store::save_settings(&app, &current_settings) {
                return CommandResponse::err("settings_save_failed", error);
            }

            CommandResponse::ok(OverlayWindowState { visible })
        }
        Err(error) => CommandResponse::err("overlay_toggle_failed", error),
    }
}

#[tauri::command]
pub fn start_asr_job(
    app: AppHandle,
    state: State<'_, AppState>,
    audio_path: String,
) -> CommandResponse<StartAsrJobOutput> {
    let input = StartAsrJobInput { audio_path };

    match asr::start_job(app, state.active_asr_job.clone(), input) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("asr_start_failed", error),
    }
}

#[tauri::command]
pub fn get_default_model_status(app: AppHandle) -> CommandResponse<DefaultModelStatus> {
    match model::get_default_model_status(&app) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("model_status_failed", error),
    }
}

#[tauri::command]
pub fn download_default_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<DownloadModelOutput> {
    match model::download_default_model(app, state.active_model_download.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("model_download_failed", error),
    }
}

#[tauri::command]
pub fn get_library_state(app: AppHandle) -> CommandResponse<LibraryState> {
    match media::get_library_state(&app) {
        Ok(state) => CommandResponse::ok(state),
        Err(error) => CommandResponse::err("library_state_failed", error),
    }
}

#[tauri::command]
pub fn import_media(app: AppHandle, source_path: String) -> CommandResponse<MediaItem> {
    match media::import_media(&app, ImportMediaInput { source_path }) {
        Ok(item) => CommandResponse::ok(item),
        Err(error) => CommandResponse::err("import_media_failed", error),
    }
}

#[tauri::command]
pub fn delete_media(app: AppHandle, media_id: String) -> CommandResponse<bool> {
    match media::delete_media(&app, &media_id) {
        Ok(_) => CommandResponse::ok(true),
        Err(error) => CommandResponse::err("delete_media_failed", error),
    }
}

#[tauri::command]
pub fn update_media_subtitle(
    app: AppHandle,
    media_id: String,
    subtitle_path: String,
) -> CommandResponse<MediaItem> {
    match media::update_media_subtitle(&app, &media_id, &subtitle_path) {
        Ok(item) => CommandResponse::ok(item),
        Err(error) => CommandResponse::err("update_media_subtitle_failed", error),
    }
}

#[tauri::command]
pub fn record_playback(app: AppHandle, media_id: String) -> CommandResponse<PlaybackHistoryItem> {
    match media::record_playback(&app, &media_id) {
        Ok(item) => CommandResponse::ok(item),
        Err(error) => CommandResponse::err("record_playback_failed", error),
    }
}

#[tauri::command]
pub fn clear_subtitles(app: AppHandle) -> CommandResponse<CleanupResult> {
    match storage::clear_subtitles(&app) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("clear_subtitles_failed", error),
    }
}

#[tauri::command]
pub fn clear_audio_cache(app: AppHandle) -> CommandResponse<CleanupResult> {
    match storage::clear_audio_cache(&app) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("clear_audio_cache_failed", error),
    }
}

#[tauri::command]
pub fn delete_default_model(app: AppHandle) -> CommandResponse<CleanupResult> {
    match storage::delete_default_model(&app) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("delete_default_model_failed", error),
    }
}

#[tauri::command]
pub fn reset_app_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<CleanupResult> {
    match storage::reset_app_data(&app) {
        Ok(result) => {
            if let Ok(mut guard) = state.settings.lock() {
                *guard = AppSettings::default();
            }
            CommandResponse::ok(result)
        }
        Err(error) => CommandResponse::err("reset_app_data_failed", error),
    }
}
