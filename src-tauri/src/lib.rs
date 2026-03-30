mod asr;
mod commands;
mod error;
mod media;
mod model;
mod shutdown;
mod sidecar;
mod state;
mod storage;
mod store;
mod subtitle;
mod window;
mod yt_dlp;

use state::{AppSettings, AppState};
use std::sync::atomic::Ordering;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

const TRAY_SHOW_MAIN_ID: &str = "tray_show_main";
const TRAY_EXIT_APP_ID: &str = "tray_exit_app";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    if std::env::var_os("WEBVIEW2_DEFAULT_BACKGROUND_COLOR").is_none() {
        std::env::set_var("WEBVIEW2_DEFAULT_BACKGROUND_COLOR", "00000000");
    }

    let app = tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.shutdown_confirmed.load(Ordering::SeqCst) {
                    return;
                }

                api.prevent_close();
                let _ = window::hide_main_window(&window.app_handle());
            }
        })
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

            #[cfg(target_os = "windows")]
            if let Some(main_window) = app_handle.get_webview_window("main") {
                main_window
                    .set_decorations(false)
                    .map_err(std::io::Error::other)?;
                main_window
                    .set_shadow(false)
                    .map_err(std::io::Error::other)?;
            }

            window::ensure_overlay_window(&app_handle, &settings).map_err(std::io::Error::other)?;

            let tray_show_main = MenuItem::with_id(
                &app_handle,
                TRAY_SHOW_MAIN_ID,
                "显示主窗口",
                true,
                None::<&str>,
            )
            .map_err(std::io::Error::other)?;
            let tray_exit_app =
                MenuItem::with_id(&app_handle, TRAY_EXIT_APP_ID, "退出", true, None::<&str>)
                    .map_err(std::io::Error::other)?;
            let tray_menu = Menu::with_items(&app_handle, &[&tray_show_main, &tray_exit_app])
                .map_err(std::io::Error::other)?;
            let tray_icon = app_handle
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("未找到应用图标"))?;

            if app_handle.tray_by_id("main-tray").is_none() {
                TrayIconBuilder::with_id("main-tray")
                    .icon(tray_icon)
                    .tooltip("muyu")
                    .menu(&tray_menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| {
                        if event.id() == TRAY_SHOW_MAIN_ID {
                            let _ = window::show_main_window(app);
                            return;
                        }

                        if event.id() == TRAY_EXIT_APP_ID {
                            let state = app.state::<AppState>();
                            let summary = shutdown::get_task_summary(&state);
                            let _ =
                                app.emit_to("main", shutdown::APP_CLOSE_REQUESTED_EVENT, summary);
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let _ = window::show_main_window(tray.app_handle());
                        }
                    })
                    .build(&app_handle)
                    .map_err(std::io::Error::other)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::update_playback_state,
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
            commands::cancel_model_download,
            commands::pause_model_download,
            commands::resume_model_download,
            commands::delete_model,
            commands::get_yt_dlp_status,
            commands::update_yt_dlp,
            commands::get_library_state,
            commands::probe_media_path,
            commands::import_media,
            commands::import_online_media,
            commands::delete_media,
            commands::update_media_subtitle,
            commands::get_subtitle_document,
            commands::save_subtitle_document,
            commands::translate_media_subtitle,
            commands::get_shutdown_task_summary,
            commands::shutdown_and_exit,
            commands::record_playback,
            commands::remove_playback_item,
            commands::clear_subtitles,
            commands::clear_audio_cache,
            commands::clear_media_library,
            commands::delete_default_model,
            commands::reset_app_data
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        shutdown::handle_run_event(app_handle, &event);
    });
}
