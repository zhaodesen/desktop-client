use reqwest::{
    blocking::Client,
    header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE},
};
use serde::{Deserialize, Serialize};
use std::{
    cmp,
    fs::{self, File},
    io::{copy, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::ModelDownloadState;

// ---------------------------------------------------------------------------
// Model registry
// ---------------------------------------------------------------------------

const DEFAULT_MODEL_ENDPOINT: &str = "https://huggingface.co";
const MIRROR_MODEL_ENDPOINT: &str = "https://hf-mirror.com";
const DEFAULT_MODEL_REPO: &str = "ggerganov/whisper.cpp";
const DOWNLOAD_BUFFER_SIZE: usize = 1024 * 1024;
const PARALLEL_DOWNLOAD_CONNECTIONS: usize = 8;
const PARALLEL_DOWNLOAD_MIN_BYTES: u64 = 10 * 1024 * 1024;

/// Returns the user-configured endpoint (if any) or `None` for default behaviour.
fn resolve_custom_endpoint() -> Option<String> {
    std::env::var("MUYU_MODEL_ENDPOINT")
        .ok()
        .or_else(|| std::env::var("HF_ENDPOINT").ok())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

/// Returns the list of endpoints to try, in order.
/// If the user set a custom endpoint we only use that one;
/// otherwise we try the official endpoint first, then the mirror.
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

fn build_model_download_url(file_name: &str) -> String {
    format!(
        "{}/{}/resolve/main/{}",
        resolve_model_endpoint(),
        DEFAULT_MODEL_REPO,
        file_name
    )
}

fn build_model_download_url_with_endpoint(endpoint: &str, file_name: &str) -> String {
    format!(
        "{}/{}/resolve/main/{}",
        endpoint, DEFAULT_MODEL_REPO, file_name
    )
}

#[derive(Debug, Clone, Copy)]
struct DownloadMetadata {
    total_bytes: Option<u64>,
    supports_ranges: bool,
}

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
            download_url: build_model_download_url("ggml-tiny.bin"),
            size_mb: 75,
        },
        ModelInfo {
            id: "base".into(),
            label: "Base".into(),
            description: "基础模型，速度与质量均衡 (~142 MB)".into(),
            file_name: "ggml-base.bin".into(),
            download_url: build_model_download_url("ggml-base.bin"),
            size_mb: 142,
        },
        ModelInfo {
            id: "small".into(),
            label: "Small".into(),
            description: "中等模型，识别质量更好 (~466 MB)".into(),
            file_name: "ggml-small.bin".into(),
            download_url: build_model_download_url("ggml-small.bin"),
            size_mb: 466,
        },
        ModelInfo {
            id: "medium".into(),
            label: "Medium".into(),
            description: "较大模型，识别准确率更高 (~1.5 GB)".into(),
            file_name: "ggml-medium.bin".into(),
            download_url: build_model_download_url("ggml-medium.bin"),
            size_mb: 1500,
        },
        ModelInfo {
            id: "large-v3-turbo".into(),
            label: "Large V3 Turbo".into(),
            description: "大型模型涡轮版，质量最高 (~1.6 GB)".into(),
            file_name: "ggml-large-v3-turbo.bin".into(),
            download_url: build_model_download_url("ggml-large-v3-turbo.bin"),
            size_mb: 1600,
        },
    ]
}

fn find_model_info(model_id: &str) -> Result<ModelInfo, String> {
    get_available_models()
        .into_iter()
        .find(|model| model.id == model_id)
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
pub struct CancelModelDownloadOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseModelDownloadOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeModelDownloadOutput {
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

    let current_dir = std::env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
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

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(PARALLEL_DOWNLOAD_CONNECTIONS)
        .build()
        .map_err(|error| format!("创建下载客户端失败: {error}"))
}

fn fetch_download_metadata(client: &Client, url: &str) -> Result<DownloadMetadata, String> {
    let response = client
        .head(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("请求模型元数据失败: {error}"))?;

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

fn download_to_temp_file(
    client: &Client,
    app: &AppHandle,
    job: &ModelDownloadState,
    info: &ModelInfo,
    temp_path: &PathBuf,
    metadata: DownloadMetadata,
) -> Result<(), String> {
    let can_parallel = metadata.supports_ranges
        && metadata
            .total_bytes
            .map(|size| size >= PARALLEL_DOWNLOAD_MIN_BYTES)
            .unwrap_or(false);

    if can_parallel {
        emit_progress(
            app,
            &job.job_id,
            &format!("正在使用多连接下载模型 {} (~{} MB)", info.label, info.size_mb),
            Some(2.0),
        )?;
        download_parallel(client, app, job, info, temp_path, metadata)
    } else {
        download_single_stream(client, app, job, info, temp_path)
    }
}

fn download_single_stream(
    client: &Client,
    app: &AppHandle,
    job: &ModelDownloadState,
    info: &ModelInfo,
    temp_path: &PathBuf,
) -> Result<(), String> {
    let mut response = client
        .get(&info.download_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("下载模型失败: {error}"))?;

    let total_bytes = response.content_length();
    let mut file = BufWriter::with_capacity(
        DOWNLOAD_BUFFER_SIZE,
        File::create(temp_path).map_err(|error| format!("创建模型文件失败: {error}"))?,
    );
    let mut downloaded_bytes = 0u64;
    let mut last_percent = 0f32;
    let mut buffer = [0u8; DOWNLOAD_BUFFER_SIZE];

    loop {
        if job.cancel_requested.load(Ordering::SeqCst) {
            cleanup_download_path(temp_path);
            return Err("模型下载已取消".to_string());
        }

        while job.pause_requested.load(Ordering::SeqCst) {
            if job.cancel_requested.load(Ordering::SeqCst) {
                cleanup_download_path(temp_path);
                return Err("模型下载已取消".to_string());
            }
            thread::sleep(Duration::from_millis(200));
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

        emit_download_progress(
            app,
            &job.job_id,
            &info.label,
            downloaded_bytes,
            total_bytes,
            &mut last_percent,
        )?;
    }

    Ok(())
}

fn download_parallel(
    client: &Client,
    app: &AppHandle,
    job: &ModelDownloadState,
    info: &ModelInfo,
    temp_path: &PathBuf,
    metadata: DownloadMetadata,
) -> Result<(), String> {
    let total_bytes = metadata
        .total_bytes
        .ok_or_else(|| "模型大小未知，无法启用分片下载".to_string())?;
    let connection_count = cmp::max(
        2,
        cmp::min(
            PARALLEL_DOWNLOAD_CONNECTIONS,
            cmp::max(1, (total_bytes / PARALLEL_DOWNLOAD_MIN_BYTES) as usize),
        ),
    );
    let part_dir = temp_path.with_extension("download.parts");

    cleanup_download_path(&part_dir);
    fs::create_dir_all(&part_dir).map_err(|error| format!("创建临时分片目录失败: {error}"))?;

    if let Ok(mut temp_guard) = job.temp_path.lock() {
        *temp_guard = Some(part_dir.clone());
    }

    let downloaded_bytes = Arc::new(AtomicU64::new(0));
    let abort_requested = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(connection_count);

    for index in 0..connection_count {
        let start = total_bytes * index as u64 / connection_count as u64;
        let end = (total_bytes * (index as u64 + 1) / connection_count as u64).saturating_sub(1);
        let client = client.clone();
        let url = info.download_url.clone();
        let part_path = part_dir.join(format!("part-{index:02}.bin"));
        let cancel_requested = job.cancel_requested.clone();
        let pause_requested = job.pause_requested.clone();
        let abort_flag = abort_requested.clone();
        let downloaded = downloaded_bytes.clone();

        handles.push(Some(thread::spawn(move || {
            download_range_to_part(
                &client,
                &url,
                start,
                end,
                &part_path,
                cancel_requested,
                pause_requested,
                abort_flag,
                downloaded,
            )
        })));
    }

    let mut remaining = connection_count;
    let mut last_percent = 0f32;
    let mut first_error: Option<String> = None;

    while remaining > 0 {
        if job.cancel_requested.load(Ordering::SeqCst) {
            abort_requested.store(true, Ordering::SeqCst);
            break;
        }

        emit_download_progress(
            app,
            &job.job_id,
            &info.label,
            downloaded_bytes.load(Ordering::SeqCst),
            Some(total_bytes),
            &mut last_percent,
        )?;

        let mut finished_in_this_round = false;
        for handle in &mut handles {
            if handle
                .as_ref()
                .map(|worker| worker.is_finished())
                .unwrap_or(false)
            {
                let worker = handle.take().expect("finished worker must exist");
                remaining -= 1;
                finished_in_this_round = true;

                match worker.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) if error == "cancelled" => {}
                    Ok(Err(error)) => {
                        abort_requested.store(true, Ordering::SeqCst);
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                    Err(_) => {
                        abort_requested.store(true, Ordering::SeqCst);
                        if first_error.is_none() {
                            first_error = Some("分片下载线程异常退出".to_string());
                        }
                    }
                }
            }
        }

        if first_error.is_some() {
            break;
        }

        if !finished_in_this_round {
            thread::sleep(Duration::from_millis(200));
        }
    }

    abort_requested.store(true, Ordering::SeqCst);
    for handle in handles.into_iter().flatten() {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error == "cancelled" => {}
            Ok(Err(error)) if first_error.is_none() => first_error = Some(error),
            Err(_) if first_error.is_none() => {
                first_error = Some("分片下载线程异常退出".to_string())
            }
            _ => {}
        }
    }

    if job.cancel_requested.load(Ordering::SeqCst) {
        cleanup_download_path(&part_dir);
        return Err("模型下载已取消".to_string());
    }

    if let Some(error) = first_error {
        cleanup_download_path(&part_dir);
        return Err(error);
    }

    let merge_result = (|| -> Result<(), String> {
        let mut output = BufWriter::with_capacity(
            DOWNLOAD_BUFFER_SIZE,
            File::create(temp_path).map_err(|error| format!("创建模型文件失败: {error}"))?,
        );
        for index in 0..connection_count {
            let part_path = part_dir.join(format!("part-{index:02}.bin"));
            let mut input = BufReader::with_capacity(
                DOWNLOAD_BUFFER_SIZE,
                File::open(&part_path).map_err(|error| format!("读取分片文件失败: {error}"))?,
            );
            copy(&mut input, &mut output).map_err(|error| format!("合并分片文件失败: {error}"))?;
        }
        Ok(())
    })();

    cleanup_download_path(&part_dir);
    if let Err(error) = merge_result {
        cleanup_download_path(temp_path);
        return Err(error);
    }

    if let Ok(mut temp_guard) = job.temp_path.lock() {
        *temp_guard = Some(temp_path.clone());
    }

    Ok(())
}

fn download_range_to_part(
    client: &Client,
    url: &str,
    start: u64,
    end: u64,
    part_path: &PathBuf,
    cancel_requested: Arc<AtomicBool>,
    pause_requested: Arc<AtomicBool>,
    abort_requested: Arc<AtomicBool>,
    downloaded_bytes: Arc<AtomicU64>,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .header(RANGE, format!("bytes={start}-{end}"))
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("下载模型分片失败: {error}"))?;
    let mut file = BufWriter::with_capacity(
        DOWNLOAD_BUFFER_SIZE,
        File::create(part_path).map_err(|error| format!("创建分片文件失败: {error}"))?,
    );
    let mut buffer = [0u8; DOWNLOAD_BUFFER_SIZE];

    loop {
        if cancel_requested.load(Ordering::SeqCst) || abort_requested.load(Ordering::SeqCst) {
            cleanup_download_path(part_path);
            return Err("cancelled".to_string());
        }

        while pause_requested.load(Ordering::SeqCst) {
            if cancel_requested.load(Ordering::SeqCst) || abort_requested.load(Ordering::SeqCst) {
                cleanup_download_path(part_path);
                return Err("cancelled".to_string());
            }
            thread::sleep(Duration::from_millis(200));
        }

        let read = response
            .read(&mut buffer)
            .map_err(|error| format!("读取模型分片数据失败: {error}"))?;
        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])
            .map_err(|error| format!("写入分片文件失败: {error}"))?;
        downloaded_bytes.fetch_add(read as u64, Ordering::SeqCst);
    }

    Ok(())
}

fn emit_download_progress(
    app: &AppHandle,
    job_id: &str,
    label: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    last_percent: &mut f32,
) -> Result<(), String> {
    if let Some(total_bytes) = total_bytes.filter(|size| *size > 0) {
        let percent = ((downloaded_bytes as f64 / total_bytes as f64) * 100.0).min(100.0) as f32;
        if percent >= *last_percent + 3.0 || percent >= 100.0 {
            emit_progress(
                app,
                job_id,
                &format!("正在下载模型 {}... {:.0}%", label, percent),
                Some(percent),
            )?;
            *last_percent = percent;
        }
    }

    Ok(())
}

fn cleanup_download_path(path: &PathBuf) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Get the status of a single model by ID.
pub fn get_model_status(app: &AppHandle, model_id: &str) -> Result<ModelStatus, String> {
    let info = find_model_info(model_id)?;

    if let Some(candidate) = resolve_model_candidates(app, &info.file_name)?
        .into_iter()
        .find(|candidate| candidate.path.exists())
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

/// Backward-compatible wrapper - returns status for the selected model.
pub fn get_default_model_status(app: &AppHandle) -> Result<ModelStatus, String> {
    let state = app.state::<crate::state::AppState>();
    let selected = state
        .settings
        .lock()
        .map(|settings| settings.selected_model.clone())
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

/// Download a specific model by ID.
pub fn download_model(
    app: AppHandle,
    model_id: String,
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<DownloadModelOutput, String> {
    let info = find_model_info(&model_id)?;

    if get_model_status(&app, &model_id)?.installed {
        return Err(format!("模型 {} 已存在，无需重复下载", info.label));
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

/// Cancel the active model download.
pub fn cancel_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<CancelModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的下载任务".to_string())?
    };

    job.cancel_requested.store(true, Ordering::SeqCst);
    Ok(CancelModelDownloadOutput { job_id: job.job_id })
}

/// Pause the active model download.
pub fn pause_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<PauseModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的下载任务".to_string())?
    };

    job.pause_requested.store(true, Ordering::SeqCst);
    Ok(PauseModelDownloadOutput { job_id: job.job_id })
}

/// Resume the active model download.
pub fn resume_model_download(
    active_job: Arc<Mutex<Option<ModelDownloadState>>>,
) -> Result<ResumeModelDownloadOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "模型下载状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的下载任务".to_string())?
    };

    job.pause_requested.store(false, Ordering::SeqCst);
    Ok(ResumeModelDownloadOutput { job_id: job.job_id })
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
            .map(|mut entries| entries.next().is_none())
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
            &format!("正在下载模型 {} (~{} MB)，这一步可能需要一点时间", info.label, info.size_mb),
            Some(2.0),
        )?;

        let client = build_http_client()?;
        let endpoints = resolve_model_endpoints();
        let mut last_error: Option<String> = None;

        for (index, endpoint) in endpoints.iter().enumerate() {
            let url = build_model_download_url_with_endpoint(endpoint, &info.file_name);

            if index > 0 {
                emit_progress(
                    &app,
                    &job.job_id,
                    &format!("正在切换到镜像源重试下载模型 {}", info.label),
                    Some(1.0),
                )?;
            }

            let mut download_info = info.clone();
            download_info.download_url = url.clone();

            let metadata = match fetch_download_metadata(&client, &url) {
                Ok(meta) => meta,
                Err(error) => {
                    last_error = Some(error);
                    continue;
                }
            };

            match download_to_temp_file(&client, &app, &job, &download_info, &temp_path, metadata) {
                Ok(()) => {
                    last_error = None;
                    break;
                }
                Err(error) => {
                    cleanup_download_path(&temp_path);
                    // 用户主动取消时不再重试
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
