use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::ModelDownloadState;

// ---------------------------------------------------------------------------
// Model registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    pub file_name: String,
    pub download_url: String,
    /// Approximate download size in megabytes (for display only).
    pub size_mb: u32,
}

/// Returns the built-in list of available whisper models.
pub fn get_available_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "tiny".into(),
            label: "Tiny".into(),
            description: "最小模型，速度最快，适合低配设备 (~75 MB)".into(),
            file_name: "ggml-tiny.bin".into(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
                .into(),
            size_mb: 75,
        },
        ModelInfo {
            id: "base".into(),
            label: "Base".into(),
            description: "基础模型，速度与质量均衡 (~142 MB)".into(),
            file_name: "ggml-base.bin".into(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
                .into(),
            size_mb: 142,
        },
        ModelInfo {
            id: "small".into(),
            label: "Small".into(),
            description: "中等模型，识别质量更好 (~466 MB)".into(),
            file_name: "ggml-small.bin".into(),
            download_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            size_mb: 466,
        },
        ModelInfo {
            id: "medium".into(),
            label: "Medium".into(),
            description: "较大模型，识别准确度高 (~1.5 GB)".into(),
            file_name: "ggml-medium.bin".into(),
            download_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
            size_mb: 1500,
        },
        ModelInfo {
            id: "large-v3-turbo".into(),
            label: "Large V3 Turbo".into(),
            description: "大型模型涡轮版，最高质量 (~1.6 GB)".into(),
            file_name: "ggml-large-v3-turbo.bin".into(),
            download_url:
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
                    .into(),
            size_mb: 1600,
        },
    ]
}

fn find_model_info(model_id: &str) -> Result<ModelInfo, String> {
    get_available_models()
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("未知的模型 ID: {model_id}"))
}

// ---------------------------------------------------------------------------
// Status types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub model_id: String,
    pub installed: bool,
    pub path: Option<String>,
    pub source: String,
    pub size_bytes: Option<u64>,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllModelsStatus {
    pub models: Vec<ModelStatus>,
}

// Keep the old type as an alias so existing callers still compile.
pub type DefaultModelStatus = ModelStatus;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadModelOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadStartedPayload {
    pub job_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgressPayload {
    pub job_id: String,
    pub message: String,
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadCompletedPayload {
    pub job_id: String,
    pub status: ModelStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadFailedPayload {
    pub job_id: String,
    pub code: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

struct ModelCandidate {
    path: PathBuf,
    source: String,
}

fn resolve_model_path(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?;

    Ok(app_data_dir.join("models").join(file_name))
}

fn resolve_model_candidates(
    app: &AppHandle,
    file_name: &str,
) -> Result<Vec<ModelCandidate>, String> {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var("WHISPER_MODEL_PATH") {
        candidates.push(ModelCandidate {
            path: PathBuf::from(path),
            source: "env".into(),
        });
    }

    candidates.push(ModelCandidate {
        path: resolve_model_path(app, file_name)?,
        source: "appData".into(),
    });

    let current_dir =
        std::env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    candidates.push(ModelCandidate {
        path: current_dir.join("models").join(file_name),
        source: "project".into(),
    });
    candidates.push(ModelCandidate {
        path: current_dir.join("src-tauri").join("models").join(file_name),
        source: "project".into(),
    });

    Ok(candidates)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Get the status of a single model by ID.
pub fn get_model_status(app: &AppHandle, model_id: &str) -> Result<ModelStatus, String> {
    let info = find_model_info(model_id)?;

    if let Some(candidate) = resolve_model_candidates(app, &info.file_name)?
        .into_iter()
        .find(|c| c.path.exists())
    {
        let metadata = fs::metadata(&candidate.path)
            .map_err(|error| format!("读取模型文件信息失败: {error}"))?;

        return Ok(ModelStatus {
            model_id: info.id,
            installed: true,
            path: Some(candidate.path.display().to_string()),
            source: candidate.source,
            size_bytes: Some(metadata.len()),
            download_url: info.download_url,
        });
    }

    Ok(ModelStatus {
        model_id: info.id,
        installed: false,
        path: None,
        source: "missing".into(),
        size_bytes: None,
        download_url: info.download_url,
    })
}

/// Backward-compatible wrapper — returns status for the "base" model.
pub fn get_default_model_status(app: &AppHandle) -> Result<ModelStatus, String> {
    // Check settings for selected model, fallback to "base"
    let state = app.state::<crate::state::AppState>();
    let selected = state
        .settings
        .lock()
        .map(|s| s.selected_model.clone())
        .unwrap_or_else(|_| "base".into());
    get_model_status(app, &selected)
}

/// Get the status of every model in the registry.
pub fn get_all_models_status(app: &AppHandle) -> Result<AllModelsStatus, String> {
    let models = get_available_models()
        .into_iter()
        .map(|info| get_model_status(app, &info.id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AllModelsStatus { models })
}

/// Resolve the path of the currently-selected model on disk.
pub fn resolve_default_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let state = app.state::<crate::state::AppState>();
    let selected = state
        .settings
        .lock()
        .map(|s| s.selected_model.clone())
        .unwrap_or_else(|_| "base".into());
    let info = find_model_info(&selected)?;
    resolve_model_path(app, &info.file_name)
}

/// Download a specific model by ID.
pub fn download_model(
    app: AppHandle,
    model_id: String,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<DownloadModelOutput, String> {
    let info = find_model_info(&model_id)?;

    if get_model_status(&app, &model_id)?.installed {
        return Err(format!("模型 {} 已经存在，无需重复下载", info.label));
    }

    let mut guard = active_job
        .lock()
        .map_err(|_| "模型下载状态锁已损坏".to_string())?;

    if guard.is_some() {
        return Err("已有模型下载任务正在运行".to_string());
    }

    let job_id = generate_job_id();
    let job_state = ModelDownloadState::new(job_id.clone());
    let spawned_job = job_state.clone();
    *guard = Some(job_state);

    let app_handle = app.clone();
    let active_job_ref = active_job.clone();
    thread::spawn(move || {
        run_download(app_handle, active_job_ref, spawned_job, info);
    });

    Ok(DownloadModelOutput { job_id })
}

/// Backward-compatible wrapper.
pub fn download_default_model(
    app: AppHandle,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<DownloadModelOutput, String> {
    download_model(app, "base".into(), active_job)
}

/// Delete a specific model by ID.
pub fn delete_model(
    app: &AppHandle,
    model_id: &str,
) -> Result<crate::storage::CleanupResult, String> {
    let info = find_model_info(model_id)?;
    let path = resolve_model_path(app, &info.file_name)?;

    if !path.exists() {
        return Ok(crate::storage::CleanupResult {
            deleted_files: 0,
            deleted_dirs: 0,
        });
    }

    fs::remove_file(&path).map_err(|error| format!("删除模型 {} 失败: {error}", info.label))?;

    // Prune empty parent dirs up to app data dir
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?;
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == app_data_dir {
            break;
        }
        let is_empty = fs::read_dir(dir)
            .map(|mut rd| rd.next().is_none())
            .unwrap_or(false);
        if is_empty {
            let _ = fs::remove_dir(dir);
        } else {
            break;
        }
        current = dir.parent();
    }

    Ok(crate::storage::CleanupResult {
        deleted_files: 1,
        deleted_dirs: 0,
    })
}

// ---------------------------------------------------------------------------
// Download worker
// ---------------------------------------------------------------------------

fn run_download(
    app: AppHandle,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
    job: ModelDownloadState,
    info: ModelInfo,
) {
    let model_id = info.id.clone();
    let emit_failed = |message: String| {
        let _ = app.emit_to(
            "main",
            "model://download-failed",
            ModelDownloadFailedPayload {
                job_id: job.job_id.clone(),
                code: "model_download_failed".into(),
                message,
            },
        );
    };

    let result = (|| -> Result<(), String> {
        app.emit_to(
            "main",
            "model://download-started",
            ModelDownloadStartedPayload {
                job_id: job.job_id.clone(),
                model_id: model_id.clone(),
            },
        )
        .map_err(|error| format!("发送模型下载开始事件失败: {error}"))?;

        emit_progress(
            &app,
            &job.job_id,
            &format!("正在请求模型 {} 下载地址", info.label),
            Some(0.0),
        )?;
        let target_path = resolve_model_path(&app, &info.file_name)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建模型目录失败: {error}"))?;
        }

        let temp_path = target_path.with_extension("bin.download");
        if let Ok(mut temp_guard) = job.temp_path.lock() {
            *temp_guard = Some(temp_path.clone());
        }
        emit_progress(
            &app,
            &job.job_id,
            &format!(
                "正在下载模型 {} (~{} MB)，这一步可能需要一点时间",
                info.label, info.size_mb
            ),
            Some(2.0),
        )?;
        let mut response = get(&info.download_url)
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("下载模型失败: {error}"))?;

        let mut file =
            File::create(&temp_path).map_err(|error| format!("创建模型文件失败: {error}"))?;
        let total_bytes = response.content_length();
        let mut downloaded_bytes = 0u64;
        let mut last_percent = 0f32;
        let mut buffer = [0u8; 1024 * 1024];

        loop {
            if job.cancel_requested.load(Ordering::SeqCst) {
                let _ = fs::remove_file(&temp_path);
                return Err("模型下载已取消".to_string());
            }

            let read = response
                .read(&mut buffer)
                .map_err(|error| format!("读取模型下载流失败: {error}"))?;
            if read == 0 {
                break;
            }

            file.write_all(&buffer[..read])
                .map_err(|error| format!("写入模型文件失败: {error}"))?;
            downloaded_bytes += read as u64;

            if let Some(total_bytes) = total_bytes.filter(|size| *size > 0) {
                let percent =
                    ((downloaded_bytes as f64 / total_bytes as f64) * 100.0).min(100.0) as f32;
                if percent >= last_percent + 3.0 || percent >= 100.0 {
                    emit_progress(
                        &app,
                        &job.job_id,
                        &format!("正在下载模型 {}… {:.0}%", info.label, percent),
                        Some(percent),
                    )?;
                    last_percent = percent;
                }
            }
        }

        fs::rename(&temp_path, &target_path)
            .map_err(|error| format!("保存模型文件失败: {error}"))?;
        emit_progress(&app, &job.job_id, "模型下载完成，正在校验状态", Some(100.0))?;

        let status = get_model_status(&app, &model_id)?;
        app.emit_to(
            "main",
            "model://download-completed",
            ModelDownloadCompletedPayload {
                job_id: job.job_id.clone(),
                status,
            },
        )
        .map_err(|error| format!("发送模型下载完成事件失败: {error}"))?;

        Ok(())
    })();

    if let Err(error) = result {
        emit_failed(error);
    }

    if let Ok(mut guard) = active_job.lock() {
        if guard
            .as_ref()
            .map(|current| current.job_id == job.job_id)
            .unwrap_or(false)
        {
            *guard = None;
        }
    }
}

fn emit_progress(
    app: &AppHandle,
    job_id: &str,
    message: &str,
    percent: Option<f32>,
) -> Result<(), String> {
    app.emit_to(
        "main",
        "model://download-progress",
        ModelDownloadProgressPayload {
            job_id: job_id.to_string(),
            message: message.to_string(),
            percent,
        },
    )
    .map_err(|error| format!("发送模型下载进度事件失败: {error}"))
}

fn generate_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let rand: u32 = (millis as u32).wrapping_mul(2654435761);
    format!("model-{millis}-{rand:08x}")
}
