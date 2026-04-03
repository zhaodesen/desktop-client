use reqwest::{
    blocking::Client,
    header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE},
    StatusCode,
};
use serde::Serialize;
use std::{
    fs::{self},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    model::{
        CancelModelDownloadOutput, DownloadModelOutput, ModelDownloadFailedPayload,
        ModelDownloadProgressPayload, ModelDownloadStartedPayload, PauseModelDownloadOutput,
        ResumableModelDownload, ResumeModelDownloadOutput,
    },
    state::ModelDownloadState,
    storage::CleanupResult,
};

const TRANSLATION_MODEL_ID: &str = "m2m100_418m";
const TRANSLATION_MODEL_LABEL: &str = "M2M100 418M";
const TRANSLATION_TARGET_LANGUAGE: &str = "zh";
const DEFAULT_MODEL_ENDPOINT: &str = "https://huggingface.co";
const MIRROR_MODEL_ENDPOINT: &str = "https://hf-mirror.com";
const DEFAULT_MODEL_REPO: &str = "entai2965/m2m100-418M-ctranslate2";
const TRANSLATION_MODEL_DIR_ENV: &str = "MUYU_TRANSLATION_MODEL_DIR";
const TRANSLATION_MODEL_ENDPOINT_ENV: &str = "MUYU_TRANSLATION_MODEL_ENDPOINT";
const DOWNLOAD_BUFFER_SIZE: usize = 1024 * 1024;
const M2M100_LANGUAGE_CODES: [&str; 100] = [
    "af", "am", "ar", "ast", "az", "ba", "be", "bg", "bn", "br", "bs", "ca", "ceb", "cs",
    "cy", "da", "de", "el", "en", "es", "et", "fa", "ff", "fi", "fr", "fy", "ga", "gd", "gl",
    "gu", "ha", "he", "hi", "hr", "ht", "hu", "hy", "id", "ig", "ilo", "is", "it", "ja", "jv",
    "ka", "kk", "km", "kn", "ko", "lb", "lg", "ln", "lo", "lt", "lv", "mg", "mk", "ml", "mn",
    "mr", "ms", "my", "ne", "nl", "no", "ns", "oc", "or", "pa", "pl", "ps", "pt", "ro", "ru",
    "sd", "si", "sk", "sl", "so", "sq", "sr", "ss", "su", "sv", "sw", "ta", "th", "tl", "tn",
    "tr", "uk", "ur", "uz", "vi", "wo", "xh", "yi", "yo", "zh", "zu",
];

const TRANSLATION_MODEL_DOWNLOAD_STARTED_EVENT: &str = "translation-model://download-started";
const TRANSLATION_MODEL_DOWNLOAD_PROGRESS_EVENT: &str = "translation-model://download-progress";
const TRANSLATION_MODEL_DOWNLOAD_COMPLETED_EVENT: &str = "translation-model://download-completed";
const TRANSLATION_MODEL_DOWNLOAD_FAILED_EVENT: &str = "translation-model://download-failed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    pub download_url: String,
    pub size_mb: u32,
    pub source_languages: Vec<String>,
    pub target_language: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelStatus {
    pub model_id: String,
    pub installed: bool,
    pub path: Option<String>,
    pub source: String,
    pub size_bytes: Option<u64>,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationModelDownloadCompletedPayload {
    pub job_id: String,
    pub status: TranslationModelStatus,
}

#[derive(Clone, Copy)]
struct TranslationModelFile {
    relative_path: &'static str,
    approx_bytes: u64,
}

const TRANSLATION_MODEL_FILES: [TranslationModelFile; 5] = [
    TranslationModelFile {
        relative_path: "config.json",
        approx_bytes: 512,
    },
    TranslationModelFile {
        relative_path: "model.bin",
        approx_bytes: 1_940_000_000,
    },
    TranslationModelFile {
        relative_path: "sentencepiece.bpe.model",
        approx_bytes: 2_600_000,
    },
    TranslationModelFile {
        relative_path: "shared_vocabulary.json",
        approx_bytes: 2_900_000,
    },
    TranslationModelFile {
        relative_path: "vocab.json",
        approx_bytes: 3_900_000,
    },
];

#[derive(Debug, Clone)]
struct ModelCandidate {
    path: PathBuf,
    source: String,
}

#[derive(Debug, Clone, Copy)]
struct DownloadMetadata {
    total_bytes: Option<u64>,
    supports_ranges: bool,
}

pub fn get_translation_model_info() -> TranslationModelInfo {
    TranslationModelInfo {
        id: TRANSLATION_MODEL_ID.to_string(),
        label: TRANSLATION_MODEL_LABEL.to_string(),
        description: "离线中文字幕翻译模型，支持 M2M100 全部 100 种语种翻译成中文（约 1.95 GB）"
            .to_string(),
        download_url: build_repo_url(&resolve_model_endpoint()),
        size_mb: 1950,
        source_languages: M2M100_LANGUAGE_CODES
            .iter()
            .map(|code| code.to_string())
            .collect(),
        target_language: TRANSLATION_TARGET_LANGUAGE.to_string(),
    }
}

pub fn get_translation_model_status(app: &AppHandle) -> Result<TranslationModelStatus, String> {
    let info = get_translation_model_info();

    if let Some(candidate) = resolve_model_candidates(app)?
        .into_iter()
        .find(|candidate| is_valid_model_root(&candidate.path))
    {
        return Ok(TranslationModelStatus {
            model_id: info.id,
            installed: true,
            path: Some(candidate.path.display().to_string()),
            source: candidate.source,
            size_bytes: Some(dir_size(&candidate.path)?),
            download_url: info.download_url,
        });
    }

    Ok(TranslationModelStatus {
        model_id: info.id,
        installed: false,
        path: None,
        source: "missing".to_string(),
        size_bytes: None,
        download_url: info.download_url,
    })
}

pub fn download_translation_model(
    app: AppHandle,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<DownloadModelOutput, String> {
    if get_translation_model_status(&app)?.installed {
        return Err(format!(
            "翻译模型 {} 已存在，无需重复下载",
            TRANSLATION_MODEL_LABEL
        ));
    }

    let mut guard = active_job
        .lock()
        .map_err(|_| "翻译模型下载状态锁已损坏".to_string())?;
    if guard.is_some() {
        return Err("已有翻译模型下载任务正在运行".to_string());
    }

    let job_id = generate_job_id();
    let job_state = ModelDownloadState::new(job_id.clone());
    let spawned_job = job_state.clone();
    *guard = Some(job_state);

    let app_handle = app.clone();
    let active_job_ref = active_job.clone();
    thread::spawn(move || {
        run_download(app_handle, active_job_ref, spawned_job);
    });

    Ok(DownloadModelOutput { job_id })
}

pub fn cancel_translation_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<CancelModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "翻译模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的翻译模型下载任务".to_string())?
    };

    job.cancel_requested.store(true, Ordering::SeqCst);
    Ok(CancelModelDownloadOutput { job_id: job.job_id })
}

pub fn pause_translation_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<PauseModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "翻译模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的翻译模型下载任务".to_string())?
    };

    job.pause_requested.store(true, Ordering::SeqCst);
    Ok(PauseModelDownloadOutput { job_id: job.job_id })
}

pub fn resume_translation_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<ResumeModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "翻译模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的翻译模型下载任务".to_string())?
    };

    job.pause_requested.store(false, Ordering::SeqCst);
    Ok(ResumeModelDownloadOutput { job_id: job.job_id })
}

pub fn delete_translation_model(app: &AppHandle) -> Result<CleanupResult, String> {
    let target_path = resolve_target_model_root(app)?;
    if !target_path.exists() {
        return Ok(CleanupResult {
            deleted_files: 0,
            deleted_dirs: 0,
        });
    }

    let deleted_files = count_files(&target_path)?;
    let deleted_dirs = count_dirs(&target_path)? + 1;
    fs::remove_dir_all(&target_path)
        .map_err(|error| format!("删除翻译模型 {} 失败: {error}", TRANSLATION_MODEL_LABEL))?;
    cleanup_empty_model_dirs(app, target_path.parent());

    Ok(CleanupResult {
        deleted_files,
        deleted_dirs,
    })
}

pub fn get_resumable_translation_model_download(
    app: &AppHandle,
) -> Result<Option<ResumableModelDownload>, String> {
    if get_translation_model_status(app)?.installed {
        return Ok(None);
    }

    let temp_root = resolve_temp_model_root(app)?;
    if !temp_root.exists() || !has_resumable_files(&temp_root)? {
        return Ok(None);
    }

    Ok(Some(ResumableModelDownload {
        model_id: TRANSLATION_MODEL_ID.to_string(),
    }))
}

fn run_download(
    app: AppHandle,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
    job: ModelDownloadState,
) {
    let emit_failed = |message: String| {
        let _ = app.emit_to(
            "main",
            TRANSLATION_MODEL_DOWNLOAD_FAILED_EVENT,
            ModelDownloadFailedPayload {
                job_id: job.job_id.clone(),
                code: "translation_model_download_failed".to_string(),
                message,
            },
        );
    };

    let result = (|| -> Result<(), String> {
        app.emit_to(
            "main",
            TRANSLATION_MODEL_DOWNLOAD_STARTED_EVENT,
            ModelDownloadStartedPayload {
                job_id: job.job_id.clone(),
                model_id: TRANSLATION_MODEL_ID.to_string(),
            },
        )
        .map_err(|error| format!("发送翻译模型下载开始事件失败: {error}"))?;

        emit_progress(
            &app,
            &job.job_id,
            &format!("正在准备下载翻译模型 {}", TRANSLATION_MODEL_LABEL),
            Some(0.0),
        )?;

        let target_root = resolve_target_model_root(&app)?;
        let temp_root = resolve_temp_model_root(&app)?;
        if let Some(parent) = temp_root.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("创建翻译模型目录失败: {error}"))?;
        }
        if !temp_root.exists() {
            fs::create_dir_all(&temp_root)
                .map_err(|error| format!("创建翻译模型临时目录失败: {error}"))?;
        }

        if let Ok(mut temp_guard) = job.temp_path.lock() {
            *temp_guard = Some(temp_root.clone());
        }

        let client = build_http_client()?;
        let endpoints = resolve_model_endpoints();
        let mut last_error: Option<String> = None;

        for (index, endpoint) in endpoints.iter().enumerate() {
            if index > 0 {
                emit_progress(
                    &app,
                    &job.job_id,
                    &format!(
                        "正在切换到镜像源重试下载翻译模型 {}",
                        TRANSLATION_MODEL_LABEL
                    ),
                    Some(1.0),
                )?;
            }

            match download_all_files_from_endpoint(&client, endpoint, &app, &job, &temp_root) {
                Ok(()) => {
                    last_error = None;
                    break;
                }
                Err(error) => {
                    if error.contains("已取消") {
                        return Err(error);
                    }
                    last_error = Some(error);
                    continue;
                }
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        ensure_ctranslate2_vocab_layout(&temp_root)?;

        cleanup_path(&target_root);
        fs::rename(&temp_root, &target_root)
            .map_err(|error| format!("保存翻译模型文件失败: {error}"))?;
        emit_progress(
            &app,
            &job.job_id,
            "翻译模型下载完成，正在校验状态",
            Some(100.0),
        )?;

        let status = get_translation_model_status(&app)?;
        app.emit_to(
            "main",
            TRANSLATION_MODEL_DOWNLOAD_COMPLETED_EVENT,
            TranslationModelDownloadCompletedPayload {
                job_id: job.job_id.clone(),
                status,
            },
        )
        .map_err(|error| format!("发送翻译模型下载完成事件失败: {error}"))?;

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

fn download_all_files_from_endpoint(
    client: &Client,
    endpoint: &str,
    app: &AppHandle,
    job: &ModelDownloadState,
    temp_root: &Path,
) -> Result<(), String> {
    let mut file_plan = Vec::with_capacity(TRANSLATION_MODEL_FILES.len());
    let mut total_bytes = 0u64;

    for file in TRANSLATION_MODEL_FILES {
        let url = build_translation_download_url(endpoint, file.relative_path);
        let metadata = fetch_download_metadata(client, &url).unwrap_or(DownloadMetadata {
            total_bytes: Some(file.approx_bytes),
            supports_ranges: true,
        });
        total_bytes = total_bytes.saturating_add(metadata.total_bytes.unwrap_or(file.approx_bytes));
        file_plan.push((file, url, metadata));
    }

    let mut downloaded_bytes = existing_downloaded_bytes(temp_root, &file_plan)?;
    let mut last_percent = 0f32;

    for (file, url, metadata) in file_plan {
        let target_path = temp_root.join(file.relative_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("创建翻译模型文件目录失败: {error}"))?;
        }

        emit_progress(
            app,
            &job.job_id,
            &format!("正在下载翻译模型文件 {}", file.relative_path),
            Some(progress_percent(downloaded_bytes, total_bytes)),
        )?;

        let written = download_file(
            client,
            app,
            job,
            &url,
            &target_path,
            metadata,
            downloaded_bytes,
            total_bytes,
            &mut last_percent,
        )?;
        downloaded_bytes = downloaded_bytes
            .saturating_sub(existing_valid_file_bytes(&target_path, metadata.total_bytes))
            .saturating_add(written);
    }

    Ok(())
}

fn download_file(
    client: &Client,
    app: &AppHandle,
    job: &ModelDownloadState,
    url: &str,
    target_path: &Path,
    metadata: DownloadMetadata,
    completed_before: u64,
    total_bytes: u64,
    last_percent: &mut f32,
) -> Result<u64, String> {
    let mut existing_bytes = existing_valid_file_bytes(target_path, metadata.total_bytes);
    if metadata.total_bytes.map(|expected| existing_bytes == expected).unwrap_or(false) {
        return Ok(existing_bytes);
    }

    let mut append_mode = metadata.supports_ranges && existing_bytes > 0;
    let mut request = client.get(url);
    if append_mode {
        request = request.header(RANGE, format!("bytes={existing_bytes}-"));
    }
    let response = request
        .send()
        .map_err(|error| format!("下载翻译模型文件失败: {error}"))?;
    let status = response.status();
    let mut response = if append_mode {
        if status == StatusCode::PARTIAL_CONTENT {
            response
                .error_for_status()
                .map_err(|error| format!("下载翻译模型文件失败: {error}"))?
        } else {
            existing_bytes = 0;
            append_mode = false;
            cleanup_path(&target_path.to_path_buf());
            client
                .get(url)
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| format!("下载翻译模型文件失败: {error}"))?
        }
    } else {
        response
            .error_for_status()
            .map_err(|error| format!("下载翻译模型文件失败: {error}"))?
    };
    let mut file = BufWriter::with_capacity(
        DOWNLOAD_BUFFER_SIZE,
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append_mode)
            .truncate(!append_mode)
            .open(target_path)
            .map_err(|error| format!("创建翻译模型文件失败: {error}"))?,
    );
    let mut downloaded = existing_bytes;
    let mut buffer = [0u8; DOWNLOAD_BUFFER_SIZE];

    if existing_bytes > 0 {
        let percent = progress_percent(completed_before, total_bytes);
        if percent >= *last_percent + 2.0 {
          emit_progress(
              app,
              &job.job_id,
              &format!(
                  "正在继续下载翻译模型 {}... {:.0}%",
                  TRANSLATION_MODEL_LABEL, percent
              ),
              Some(percent),
          )?;
          *last_percent = percent;
        }
    }

    loop {
        if job.cancel_requested.load(Ordering::SeqCst) {
            if !should_preserve_progress(job) {
                cleanup_path(&target_path.to_path_buf());
            }
            return Err("翻译模型下载已取消".to_string());
        }

        while job.pause_requested.load(Ordering::SeqCst) {
            if job.cancel_requested.load(Ordering::SeqCst) {
                if !should_preserve_progress(job) {
                    cleanup_path(&target_path.to_path_buf());
                }
                return Err("翻译模型下载已取消".to_string());
            }
            thread::sleep(Duration::from_millis(200));
        }

        let read = response
            .read(&mut buffer)
            .map_err(|error| format!("读取翻译模型下载流失败: {error}"))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])
            .map_err(|error| format!("写入翻译模型文件失败: {error}"))?;
        downloaded = downloaded.saturating_add(read as u64);

        let current_total = completed_before
            .saturating_sub(existing_bytes)
            .saturating_add(downloaded);
        let percent = progress_percent(current_total, total_bytes);
        if percent >= *last_percent + 2.0 || percent >= 100.0 {
            emit_progress(
                app,
                &job.job_id,
                &format!(
                    "正在下载翻译模型 {}... {:.0}%",
                    TRANSLATION_MODEL_LABEL, percent
                ),
                Some(percent),
            )?;
            *last_percent = percent;
        }
    }

    file.flush()
        .map_err(|error| format!("写入翻译模型文件失败: {error}"))?;

    if let Some(expected_bytes) = metadata.total_bytes.filter(|size| *size > 0) {
        if downloaded != expected_bytes {
            if !should_preserve_progress(job) {
                cleanup_path(&target_path.to_path_buf());
            }
            return Err(format!(
                "翻译模型文件下载不完整，期望 {} 字节，实际 {} 字节",
                expected_bytes, downloaded
            ));
        }
    }

    Ok(downloaded)
}

fn emit_progress(
    app: &AppHandle,
    job_id: &str,
    message: &str,
    percent: Option<f32>,
) -> Result<(), String> {
    app.emit_to(
        "main",
        TRANSLATION_MODEL_DOWNLOAD_PROGRESS_EVENT,
        ModelDownloadProgressPayload {
            job_id: job_id.to_string(),
            message: message.to_string(),
            percent,
        },
    )
    .map_err(|error| format!("发送翻译模型下载进度事件失败: {error}"))
}

fn progress_percent(downloaded: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    ((downloaded as f64 / total as f64) * 100.0).min(100.0) as f32
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("创建翻译模型下载客户端失败: {error}"))
}

fn fetch_download_metadata(client: &Client, url: &str) -> Result<DownloadMetadata, String> {
    let response = client
        .head(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("请求翻译模型元数据失败: {error}"))?;

    let total_bytes = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0);
    let supports_ranges = response
        .headers()
        .get(ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false);

    Ok(DownloadMetadata {
        total_bytes,
        supports_ranges,
    })
}

fn resolve_custom_endpoint() -> Option<String> {
    std::env::var(TRANSLATION_MODEL_ENDPOINT_ENV)
        .ok()
        .or_else(|| std::env::var("MUYU_MODEL_ENDPOINT").ok())
        .or_else(|| std::env::var("HF_ENDPOINT").ok())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_model_endpoints() -> Vec<String> {
    if let Some(custom) = resolve_custom_endpoint() {
        vec![custom]
    } else {
        vec![
            MIRROR_MODEL_ENDPOINT.to_string(),
            DEFAULT_MODEL_ENDPOINT.to_string(),
        ]
    }
}

fn resolve_model_endpoint() -> String {
    resolve_custom_endpoint().unwrap_or_else(|| DEFAULT_MODEL_ENDPOINT.to_string())
}

fn build_repo_url(endpoint: &str) -> String {
    format!("{}/{}", endpoint.trim_end_matches('/'), DEFAULT_MODEL_REPO)
}

fn build_translation_download_url(endpoint: &str, relative_path: &str) -> String {
    format!(
        "{}/{}/resolve/main/{}",
        endpoint.trim_end_matches('/'),
        DEFAULT_MODEL_REPO,
        relative_path
    )
}

fn resolve_target_model_root(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?;
    Ok(app_data_dir
        .join("models")
        .join("translation")
        .join(TRANSLATION_MODEL_ID))
}

fn resolve_temp_model_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_target_model_root(app)?.with_extension("download"))
}

fn resolve_model_candidates(app: &AppHandle) -> Result<Vec<ModelCandidate>, String> {
    let mut candidates = Vec::new();

    if let Ok(path) = std::env::var(TRANSLATION_MODEL_DIR_ENV) {
        candidates.push(ModelCandidate {
            path: PathBuf::from(path),
            source: "env".to_string(),
        });
    }

    let app_root = resolve_target_model_root(app)?;
    candidates.push(ModelCandidate {
        path: app_root,
        source: "appData".to_string(),
    });

    let app_translation_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("models")
        .join("translation");
    candidates.push(ModelCandidate {
        path: app_translation_dir,
        source: "appData".to_string(),
    });

    let current_dir =
        std::env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    candidates.push(ModelCandidate {
        path: current_dir
            .join("models")
            .join("translation")
            .join(TRANSLATION_MODEL_ID),
        source: "project".to_string(),
    });
    candidates.push(ModelCandidate {
        path: current_dir.join("models").join("translation"),
        source: "project".to_string(),
    });
    candidates.push(ModelCandidate {
        path: current_dir
            .join("src-tauri")
            .join("models")
            .join("translation")
            .join(TRANSLATION_MODEL_ID),
        source: "project".to_string(),
    });
    candidates.push(ModelCandidate {
        path: current_dir
            .join("src-tauri")
            .join("models")
            .join("translation"),
        source: "project".to_string(),
    });

    Ok(candidates)
}

fn is_valid_model_root(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    let has_model_bin =
        path.join("model.bin").exists() || path.join("ctranslate2").join("model.bin").exists();
    let has_config =
        path.join("config.json").exists() || path.join("ctranslate2").join("config.json").exists();
    let has_sentencepiece = path.join("sentencepiece.bpe.model").exists();
    let has_shared_vocabulary = path.join("shared_vocabulary.json").exists();
    let has_source_vocabulary = path.join("source_vocabulary.json").exists();
    let has_target_vocabulary = path.join("target_vocabulary.json").exists();
    let has_vocab = path.join("vocab.json").exists();

    has_model_bin
        && has_config
        && has_sentencepiece
        && has_shared_vocabulary
        && has_source_vocabulary
        && has_target_vocabulary
        && has_vocab
}

fn ensure_ctranslate2_vocab_layout(model_root: &Path) -> Result<(), String> {
    let shared_vocabulary = model_root.join("shared_vocabulary.json");
    if !shared_vocabulary.exists() {
        return Err("翻译模型缺少 shared_vocabulary.json".to_string());
    }

    for target_name in ["source_vocabulary.json", "target_vocabulary.json"] {
        let target = model_root.join(target_name);
        if target.exists() {
            continue;
        }
        fs::copy(&shared_vocabulary, &target)
            .map_err(|error| format!("生成 {target_name} 失败: {error}"))?;
    }

    Ok(())
}

fn dir_size(path: &Path) -> Result<u64, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("读取翻译模型信息失败: {error}"))?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|error| format!("读取翻译模型目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取翻译模型目录项失败: {error}"))?;
        total = total.saturating_add(dir_size(&entry.path())?);
    }
    Ok(total)
}

fn count_files(path: &Path) -> Result<usize, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("读取翻译模型信息失败: {error}"))?;
    if metadata.is_file() {
        return Ok(1);
    }

    let mut total = 0usize;
    for entry in fs::read_dir(path).map_err(|error| format!("读取翻译模型目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取翻译模型目录项失败: {error}"))?;
        total += count_files(&entry.path())?;
    }
    Ok(total)
}

fn count_dirs(path: &Path) -> Result<usize, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("读取翻译模型信息失败: {error}"))?;
    if metadata.is_file() {
        return Ok(0);
    }

    let mut total = 0usize;
    for entry in fs::read_dir(path).map_err(|error| format!("读取翻译模型目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取翻译模型目录项失败: {error}"))?;
        let child = entry.path();
        if child.is_dir() {
            total += 1 + count_dirs(&child)?;
        }
    }
    Ok(total)
}

fn cleanup_path(path: &PathBuf) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn should_preserve_progress(job: &ModelDownloadState) -> bool {
    job.preserve_temp_on_cancel.load(Ordering::SeqCst)
}

fn existing_valid_file_bytes(path: &Path, expected_bytes: Option<u64>) -> u64 {
    let size = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    match expected_bytes {
        Some(expected) if size > expected => {
            cleanup_path(&path.to_path_buf());
            0
        }
        Some(expected) => size.min(expected),
        None => size,
    }
}

fn existing_downloaded_bytes(
    temp_root: &Path,
    plan: &[(TranslationModelFile, String, DownloadMetadata)],
) -> Result<u64, String> {
    let mut total = 0u64;
    for (file, _, metadata) in plan {
        total = total.saturating_add(existing_valid_file_bytes(
            &temp_root.join(file.relative_path),
            metadata.total_bytes,
        ));
    }
    Ok(total)
}

fn has_resumable_files(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    for entry in fs::read_dir(path).map_err(|error| format!("读取翻译模型临时目录失败: {error}"))? {
        let entry = entry.map_err(|error| format!("读取翻译模型临时目录项失败: {error}"))?;
        let child = entry.path();
        if child.is_dir() {
            if has_resumable_files(&child)? {
                return Ok(true);
            }
        } else if fs::metadata(&child).map(|meta| meta.len() > 0).unwrap_or(false) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn cleanup_empty_model_dirs(app: &AppHandle, start: Option<&Path>) {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return;
    };

    let mut current = start;
    while let Some(dir) = current {
        if dir == app_data_dir {
            break;
        }

        let is_empty = fs::read_dir(dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if is_empty {
            let _ = fs::remove_dir(dir);
        } else {
            break;
        }
        current = dir.parent();
    }
}

fn generate_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let rand: u32 = (millis as u32).wrapping_mul(2246822519);
    format!("translation-model-{millis}-{rand:08x}")
}
