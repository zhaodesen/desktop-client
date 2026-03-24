use serde::Serialize;
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, RunEvent};

use crate::{asr, sidecar, state::AppState};

pub const APP_CLOSE_REQUESTED_EVENT: &str = "app://close-requested";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownTaskSummary {
    pub has_active_tasks: bool,
    pub tasks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownCleanupOutput {
    pub cancelled_tasks: Vec<String>,
}

pub fn list_active_tasks(state: &AppState) -> Vec<String> {
    let mut tasks = Vec::new();

    if state
        .active_asr_job
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
    {
        tasks.push("字幕识别".to_string());
    }

    if state
        .active_translation_job
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
    {
        tasks.push("字幕翻译".to_string());
    }

    if state
        .active_online_import
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
    {
        tasks.push("在线视频下载".to_string());
    }

    if state
        .active_model_download
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
    {
        tasks.push("模型下载".to_string());
    }

    tasks
}

pub fn get_task_summary(state: &AppState) -> ShutdownTaskSummary {
    let tasks = list_active_tasks(state);
    ShutdownTaskSummary {
        has_active_tasks: !tasks.is_empty(),
        tasks,
    }
}

pub fn handle_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<AppState>();
            if state.shutdown_confirmed.load(Ordering::SeqCst) {
                return;
            }

            let summary = get_task_summary(&state);
            if summary.has_active_tasks {
                api.prevent_exit();
                let _ = app.emit_to("main", APP_CLOSE_REQUESTED_EVENT, summary);
            }
        }
        RunEvent::Exit => {
            let state = app.state::<AppState>();
            state.shutdown_confirmed.store(true, Ordering::SeqCst);
        }
        _ => {}
    }
}

pub fn prepare_for_exit(app: &AppHandle, state: &AppState) -> ShutdownCleanupOutput {
    state.shutdown_confirmed.store(true, Ordering::SeqCst);

    let mut cancelled_tasks = Vec::new();

    if state
        .active_asr_job
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
    {
        let _ = asr::cancel_job(state.active_asr_job.clone());
        cancelled_tasks.push("字幕识别".to_string());
    }

    if let Ok(mut guard) = state.active_translation_job.lock() {
        if let Some(task) = guard.take() {
            let _ = sidecar::kill_process(task.pid);
            cancelled_tasks.push(task.label.to_string());
        }
    }

    if let Ok(mut guard) = state.active_online_import.lock() {
        if let Some(task) = guard.take() {
            let _ = sidecar::kill_process(task.pid);
            cancelled_tasks.push(task.label.to_string());
        }
    }

    if let Ok(guard) = state.active_model_download.lock() {
        if let Some(task) = guard.as_ref() {
            task.cancel_requested.store(true, Ordering::SeqCst);
            cancelled_tasks.push("模型下载".to_string());
        }
    }

    wait_for_shutdown_cleanup(state);

    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.close();
    }

    ShutdownCleanupOutput { cancelled_tasks }
}

pub fn exit_after_cleanup(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(120));
        app.exit(0);
    });
}

fn wait_for_shutdown_cleanup(state: &AppState) {
    let deadline = Instant::now() + Duration::from_millis(800);
    while Instant::now() < deadline {
        let translation_done = state
            .active_translation_job
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(true);
        let import_done = state
            .active_online_import
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(true);
        let model_done = state
            .active_model_download
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(true);
        let asr_done = state
            .active_asr_job
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(true);

        if translation_done && import_done && model_done && asr_done {
            break;
        }

        thread::sleep(Duration::from_millis(40));
    }
}
