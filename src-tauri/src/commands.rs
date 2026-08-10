use crate::{
    asr::{self, CancelAsrJobOutput, StartAsrJobInput, StartAsrJobOutput},
    error::CommandResponse,
    media::{
        self, ImportMediaInput, LibraryState, MediaDebugProbe, MediaItem, PlaybackHistoryItem,
    },
    model::{
        self, AllModelsStatus, CancelModelDownloadOutput, DefaultModelStatus, DownloadModelOutput,
        ModelInfo, ModelStatus, PauseModelDownloadOutput, ResumableModelDownload,
        ResumeModelDownloadOutput,
    },
    shutdown::{self, ShutdownCleanupOutput, ShutdownTaskSummary},
    state::{AppSettings, AppState, OverlayWindowState, PlaybackState},
    storage::{self, CleanupResult},
    store,
    subtitle::{self, SubtitleCue, SubtitleDocument},
    translation_model::{self, TranslationModelInfo, TranslationModelStatus},
    window,
};
use tauri::{async_runtime, AppHandle, Manager, State};

async fn run_blocking_task<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("后台任务执行失败: {error}"))?
}

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
    mut settings: AppSettings,
) -> CommandResponse<AppSettings> {
    let visibility_result = if settings.overlay_visible {
        window::show_overlay(&app)
    } else {
        window::hide_overlay(&app)
    };

    if let Err(error) = visibility_result {
        return CommandResponse::err("overlay_visibility_failed", error);
    }

    if settings.playback_state.is_none() {
        match state.settings.lock() {
            Ok(guard) => {
                settings.playback_state = guard.playback_state.clone();
            }
            Err(_) => {
                return CommandResponse::err("settings_lock_failed", "读取当前播放状态失败");
            }
        }
    }

    match update_settings_in_state(&app, &state, settings) {
        Ok(saved) => CommandResponse::ok(saved),
        Err(error) => CommandResponse::err("settings_save_failed", error),
    }
}

#[tauri::command]
pub fn update_playback_state(
    app: AppHandle,
    state: State<'_, AppState>,
    playback_state: Option<PlaybackState>,
) -> CommandResponse<Option<PlaybackState>> {
    let current_settings = match state.settings.lock() {
        Ok(mut guard) => {
            guard.playback_state = playback_state.clone();
            guard.clone()
        }
        Err(_) => {
            return CommandResponse::err("settings_lock_failed", "更新播放状态失败");
        }
    };

    if let Err(error) = store::save_settings(&app, &current_settings) {
        return CommandResponse::err("settings_save_failed", error);
    }

    CommandResponse::ok(playback_state)
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
pub fn cancel_asr_job(state: State<'_, AppState>) -> CommandResponse<CancelAsrJobOutput> {
    match asr::cancel_job(state.active_asr_job.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("asr_cancel_failed", error),
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
pub fn get_available_models() -> CommandResponse<Vec<ModelInfo>> {
    CommandResponse::ok(model::get_available_models())
}

#[tauri::command]
pub fn get_all_models_status(app: AppHandle) -> CommandResponse<AllModelsStatus> {
    match model::get_all_models_status(&app) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("all_models_status_failed", error),
    }
}

#[tauri::command]
pub fn get_model_status(app: AppHandle, model_id: String) -> CommandResponse<ModelStatus> {
    match model::get_model_status(&app, &model_id) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("model_status_failed", error),
    }
}

#[tauri::command]
pub fn get_resumable_model_download(
    app: AppHandle,
) -> CommandResponse<Option<ResumableModelDownload>> {
    match model::get_resumable_model_download(&app) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("resumable_model_download_failed", error),
    }
}

#[tauri::command]
pub fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResponse<DownloadModelOutput> {
    match model::download_model(app, model_id, state.active_model_download.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("model_download_failed", error),
    }
}

#[tauri::command]
pub fn cancel_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<CancelModelDownloadOutput> {
    match model::cancel_model_download(state.active_model_download.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("cancel_model_download_failed", error),
    }
}

#[tauri::command]
pub fn pause_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<PauseModelDownloadOutput> {
    match model::pause_model_download(state.active_model_download.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("pause_model_download_failed", error),
    }
}

#[tauri::command]
pub fn resume_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<ResumeModelDownloadOutput> {
    match model::resume_model_download(state.active_model_download.clone()) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("resume_model_download_failed", error),
    }
}

#[tauri::command]
pub fn delete_model(app: AppHandle, model_id: String) -> CommandResponse<CleanupResult> {
    match model::delete_model(&app, &model_id) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("delete_model_failed", error),
    }
}

#[tauri::command]
pub fn get_translation_model_info() -> CommandResponse<TranslationModelInfo> {
    CommandResponse::ok(translation_model::get_translation_model_info())
}

#[tauri::command]
pub fn get_translation_model_status(app: AppHandle) -> CommandResponse<TranslationModelStatus> {
    match translation_model::get_translation_model_status(&app) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("translation_model_status_failed", error),
    }
}

#[tauri::command]
pub fn get_resumable_translation_model_download(
    app: AppHandle,
) -> CommandResponse<Option<ResumableModelDownload>> {
    match translation_model::get_resumable_translation_model_download(&app) {
        Ok(status) => CommandResponse::ok(status),
        Err(error) => CommandResponse::err("resumable_translation_model_download_failed", error),
    }
}

#[tauri::command]
pub fn download_translation_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<DownloadModelOutput> {
    match translation_model::download_translation_model(
        app,
        state.active_translation_model_download.clone(),
    ) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("translation_model_download_failed", error),
    }
}

#[tauri::command]
pub fn cancel_translation_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<CancelModelDownloadOutput> {
    match translation_model::cancel_translation_model_download(
        state.active_translation_model_download.clone(),
    ) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("cancel_translation_model_download_failed", error),
    }
}

#[tauri::command]
pub fn pause_translation_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<PauseModelDownloadOutput> {
    match translation_model::pause_translation_model_download(
        state.active_translation_model_download.clone(),
    ) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("pause_translation_model_download_failed", error),
    }
}

#[tauri::command]
pub fn resume_translation_model_download(
    state: State<'_, AppState>,
) -> CommandResponse<ResumeModelDownloadOutput> {
    match translation_model::resume_translation_model_download(
        state.active_translation_model_download.clone(),
    ) {
        Ok(output) => CommandResponse::ok(output),
        Err(error) => CommandResponse::err("resume_translation_model_download_failed", error),
    }
}

#[tauri::command]
pub fn delete_translation_model(app: AppHandle) -> CommandResponse<CleanupResult> {
    match translation_model::delete_translation_model(&app) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("delete_translation_model_failed", error),
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
pub fn probe_media_path(app: AppHandle, path: String) -> CommandResponse<MediaDebugProbe> {
    match media::probe_media_path(&app, &path) {
        Ok(probe) => CommandResponse::ok(probe),
        Err(error) => CommandResponse::err("probe_media_path_failed", error),
    }
}

#[tauri::command]
pub async fn import_media(app: AppHandle, source_path: String) -> CommandResponse<MediaItem> {
    let app_handle = app.clone();

    match run_blocking_task(move || {
        media::import_media(&app_handle, ImportMediaInput { source_path })
    })
    .await
    {
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
pub fn get_subtitle_document(
    app: AppHandle,
    media_id: String,
) -> CommandResponse<SubtitleDocument> {
    match subtitle::get_subtitle_document(&app, &media_id) {
        Ok(document) => CommandResponse::ok(document),
        Err(error) => CommandResponse::err("get_subtitle_document_failed", error),
    }
}

#[tauri::command]
pub fn save_subtitle_document(
    app: AppHandle,
    media_id: String,
    title: String,
    cues: Vec<SubtitleCue>,
) -> CommandResponse<SubtitleDocument> {
    match subtitle::save_subtitle_document(&app, &media_id, &title, cues) {
        Ok(document) => CommandResponse::ok(document),
        Err(error) => CommandResponse::err("save_subtitle_document_failed", error),
    }
}

#[tauri::command]
pub async fn translate_media_subtitle(
    app: AppHandle,
    media_id: String,
    source_language: Option<String>,
) -> CommandResponse<SubtitleDocument> {
    let app_handle = app.clone();
    let active_translation_job = app.state::<AppState>().active_translation_job.clone();

    match run_blocking_task(move || {
        subtitle::translate_media_subtitle(
            &app_handle,
            active_translation_job,
            &media_id,
            source_language.as_deref(),
        )
    })
    .await
    {
        Ok(document) => CommandResponse::ok(document),
        Err(error) => CommandResponse::err("translate_media_subtitle_failed", error),
    }
}

#[tauri::command]
pub fn get_shutdown_task_summary(
    state: State<'_, AppState>,
) -> CommandResponse<ShutdownTaskSummary> {
    CommandResponse::ok(shutdown::get_task_summary(&state))
}

#[tauri::command]
pub fn shutdown_and_exit(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResponse<ShutdownCleanupOutput> {
    let output = shutdown::prepare_for_exit(&app, &state);
    shutdown::exit_after_cleanup(app);
    CommandResponse::ok(output)
}

#[tauri::command]
pub fn record_playback(app: AppHandle, media_id: String) -> CommandResponse<PlaybackHistoryItem> {
    match media::record_playback(&app, &media_id) {
        Ok(item) => CommandResponse::ok(item),
        Err(error) => CommandResponse::err("record_playback_failed", error),
    }
}

#[tauri::command]
pub fn prepend_playback_item(
    app: AppHandle,
    media_id: String,
) -> CommandResponse<PlaybackHistoryItem> {
    match media::prepend_playback_item(&app, &media_id) {
        Ok(item) => CommandResponse::ok(item),
        Err(error) => CommandResponse::err("prepend_playback_item_failed", error),
    }
}

#[tauri::command]
pub fn remove_playback_item(app: AppHandle, media_id: String) -> CommandResponse<bool> {
    match media::remove_playback_item(&app, &media_id) {
        Ok(_) => CommandResponse::ok(true),
        Err(error) => CommandResponse::err("remove_playback_item_failed", error),
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
pub fn clear_media_library(app: AppHandle) -> CommandResponse<CleanupResult> {
    match storage::clear_media_library(&app) {
        Ok(result) => CommandResponse::ok(result),
        Err(error) => CommandResponse::err("clear_media_library_failed", error),
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
