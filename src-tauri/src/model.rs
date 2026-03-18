use reqwest::blocking::get;
use serde::Serialize;
use std::{
    fs::{self, File},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_MODEL_ID: &str = "base";
const DEFAULT_MODEL_FILE_NAME: &str = "ggml-base.bin";
const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultModelStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub source: &'static str,
    pub size_bytes: Option<u64>,
    pub download_url: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadModelOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadStartedPayload {
    pub job_id: String,
    pub model_id: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgressPayload {
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadCompletedPayload {
    pub job_id: String,
    pub status: DefaultModelStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadFailedPayload {
    pub job_id: String,
    pub message: String,
}

struct ModelCandidate {
    path: PathBuf,
    source: &'static str,
}

pub fn get_default_model_status(app: &AppHandle) -> Result<DefaultModelStatus, String> {
    if let Some(candidate) = resolve_model_candidates(app)?
        .into_iter()
        .find(|candidate| candidate.path.exists())
    {
        let metadata = fs::metadata(&candidate.path)
            .map_err(|error| format!("读取模型文件信息失败: {error}"))?;

        return Ok(DefaultModelStatus {
            installed: true,
            path: Some(candidate.path.display().to_string()),
            source: candidate.source,
            size_bytes: Some(metadata.len()),
            download_url: DEFAULT_MODEL_URL,
        });
    }

    Ok(DefaultModelStatus {
        installed: false,
        path: None,
        source: "missing",
        size_bytes: None,
        download_url: DEFAULT_MODEL_URL,
    })
}

pub fn download_default_model(
    app: AppHandle,
    active_job: Arc<Mutex<Option<String>>>,
) -> Result<DownloadModelOutput, String> {
    if get_default_model_status(&app)?.installed {
        return Err("默认模型已经存在，无需重复下载".to_string());
    }

    let mut guard = active_job
        .lock()
        .map_err(|_| "模型下载状态锁已损坏".to_string())?;

    if guard.is_some() {
        return Err("已有模型下载任务正在运行".to_string());
    }

    let job_id = generate_job_id();
    let spawned_job_id = job_id.clone();
    *guard = Some(job_id.clone());

    let app_handle = app.clone();
    let active_job_ref = active_job.clone();
    thread::spawn(move || {
        run_download(app_handle, active_job_ref, spawned_job_id);
    });

    Ok(DownloadModelOutput { job_id })
}

pub fn resolve_default_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?;

    Ok(app_data_dir.join("models").join(DEFAULT_MODEL_FILE_NAME))
}

fn resolve_model_candidates(app: &AppHandle) -> Result<Vec<ModelCandidate>, String> {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var("WHISPER_MODEL_PATH") {
        candidates.push(ModelCandidate {
            path: PathBuf::from(path),
            source: "env",
        });
    }

    candidates.push(ModelCandidate {
        path: resolve_default_model_path(app)?,
        source: "appData",
    });

    let current_dir =
        std::env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    candidates.push(ModelCandidate {
        path: current_dir.join("models").join(DEFAULT_MODEL_FILE_NAME),
        source: "project",
    });
    candidates.push(ModelCandidate {
        path: current_dir
            .join("src-tauri")
            .join("models")
            .join(DEFAULT_MODEL_FILE_NAME),
        source: "project",
    });

    Ok(candidates)
}

fn run_download(app: AppHandle, active_job: Arc<Mutex<Option<String>>>, job_id: String) {
    let emit_failed = |message: String| {
        let _ = app.emit_to(
            "main",
            "model://download-failed",
            ModelDownloadFailedPayload {
                job_id: job_id.clone(),
                message,
            },
        );
    };

    let result = (|| -> Result<(), String> {
        app.emit_to(
            "main",
            "model://download-started",
            ModelDownloadStartedPayload {
                job_id: job_id.clone(),
                model_id: DEFAULT_MODEL_ID,
            },
        )
        .map_err(|error| format!("发送模型下载开始事件失败: {error}"))?;

        emit_progress(&app, &job_id, "正在请求默认模型下载地址")?;
        let target_path = resolve_default_model_path(&app)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建模型目录失败: {error}"))?;
        }

        let temp_path = target_path.with_extension("bin.download");
        emit_progress(&app, &job_id, "正在下载默认模型，这一步可能需要一点时间")?;
        let mut response = get(DEFAULT_MODEL_URL)
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("下载模型失败: {error}"))?;

        let mut file =
            File::create(&temp_path).map_err(|error| format!("创建模型文件失败: {error}"))?;
        response
            .copy_to(&mut file)
            .map_err(|error| format!("写入模型文件失败: {error}"))?;

        fs::rename(&temp_path, &target_path)
            .map_err(|error| format!("保存模型文件失败: {error}"))?;
        emit_progress(&app, &job_id, "模型下载完成，正在校验状态")?;

        let status = get_default_model_status(&app)?;
        app.emit_to(
            "main",
            "model://download-completed",
            ModelDownloadCompletedPayload {
                job_id: job_id.clone(),
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
        *guard = None;
    }
}

fn emit_progress(app: &AppHandle, job_id: &str, message: &str) -> Result<(), String> {
    app.emit_to(
        "main",
        "model://download-progress",
        ModelDownloadProgressPayload {
            job_id: job_id.to_string(),
            message: message.to_string(),
        },
    )
    .map_err(|error| format!("发送模型下载进度事件失败: {error}"))
}

fn generate_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("model-{millis}")
}
