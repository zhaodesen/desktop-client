use serde::Serialize;
use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::model;
use crate::sidecar::{self, CommandTarget};
use crate::state::AsrJobState;

const WHISPER_LANGUAGE_MODE: &str = "auto";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAsrJobInput {
    pub audio_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAsrJobOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAsrJobOutput {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStartedPayload {
    pub job_id: String,
    pub audio_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrProgressPayload {
    pub job_id: String,
    pub stage: &'static str,
    pub message: String,
    /// 当前阶段内的进度百分比 (0.0–100.0)，None 表示不确定
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCompletedPayload {
    pub job_id: String,
    pub subtitle_path: String,
    pub wav_path: String,
    pub model_path: String,
    pub detected_language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrFailedPayload {
    pub job_id: String,
    pub code: &'static str,
    pub message: String,
}

pub fn start_job(
    app: AppHandle,
    active_job: Arc<Mutex<Option<AsrJobState>>>,
    input: StartAsrJobInput,
) -> Result<StartAsrJobOutput, String> {
    let audio_path = PathBuf::from(&input.audio_path);
    if !audio_path.exists() {
        return Err("音频文件不存在".to_string());
    }

    {
        let mut guard = active_job
            .lock()
            .map_err(|_| "识别任务状态锁已损坏".to_string())?;
        if guard.is_some() {
            return Err("已有识别任务正在运行，请等待当前任务完成".to_string());
        }

        let job_id = generate_job_id();
        let job_state = AsrJobState::new(job_id.clone());
        let spawned_job = job_state.clone();
        *guard = Some(job_state);

        let app_handle = app.clone();
        let active_job_ref = active_job.clone();
        thread::spawn(move || {
            run_pipeline(app_handle, active_job_ref, spawned_job, audio_path);
        });

        return Ok(StartAsrJobOutput { job_id });
    }
}

pub fn cancel_job(
    active_job: Arc<Mutex<Option<AsrJobState>>>,
) -> Result<CancelAsrJobOutput, String> {
    let job = {
        let guard = active_job
            .lock()
            .map_err(|_| "识别任务状态锁已损坏".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "当前没有正在运行的识别任务".to_string())?
    };

    job.cancel_requested.store(true, Ordering::SeqCst);

    if let Ok(mut child_guard) = job.active_child.lock() {
        if let Some(child) = child_guard.as_mut() {
            let _ = child.kill();
        }
    }

    Ok(CancelAsrJobOutput { job_id: job.job_id })
}

fn run_pipeline(
    app: AppHandle,
    active_job: Arc<Mutex<Option<AsrJobState>>>,
    job: AsrJobState,
    audio_path: PathBuf,
) {
    let emit_failed = |message: String| {
        let _ = app.emit_to(
            "main",
            "asr://failed",
            AsrFailedPayload {
                job_id: job.job_id.clone(),
                code: if job.cancel_requested.load(Ordering::SeqCst) {
                    "asr_cancelled"
                } else {
                    "asr_pipeline_failed"
                },
                message,
            },
        );
    };

    let result = (|| -> Result<(), String> {
        app.emit_to(
            "main",
            "asr://started",
            AsrStartedPayload {
                job_id: job.job_id.clone(),
                audio_path: audio_path.display().to_string(),
            },
        )
        .map_err(|error| format!("发送任务开始事件失败: {error}"))?;

        ensure_not_cancelled(&job)?;
        emit_progress(
            &app,
            &job.job_id,
            "preparing",
            "正在检查 ffmpeg、whisper-cli 和模型文件",
        )?;

        let whisper = locate_whisper_cli(&app)?;
        let model_path = locate_whisper_model(&app)?;
        let subtitle_dir = ensure_dir(&app, "subtitles")?;
        let subtitle_prefix =
            subtitle_dir.join(format!("{}-{}", file_stem(&audio_path), job.job_id));
        let subtitle_path = subtitle_prefix.with_extension("srt");
        let wav_path = if is_whisper_ready_wav(&audio_path)? {
            emit_progress(
                &app,
                &job.job_id,
                "preparing",
                "检测到已标准化的 WAV 音频，跳过转码",
            )?;
            audio_path.clone()
        } else {
            let ffmpeg = locate_ffmpeg(&app)?;
            let cache_audio_dir = ensure_dir(&app, "cache/audio")?;
            let wav_path = cache_audio_dir.join(format!("{}.wav", job.job_id));

            emit_progress(
                &app,
                &job.job_id,
                "preparing",
                &format!("正在使用 {} 进行音频标准化", ffmpeg.display()),
            )?;
            run_ffmpeg(&job, &ffmpeg, &audio_path, &wav_path)?;
            wav_path
        };

        ensure_not_cancelled(&job)?;
        emit_progress(
            &app,
            &job.job_id,
            "recognizing",
            &format!(
                "正在使用 {} 和模型 {} 进行离线识别",
                whisper.display(),
                model_path.display()
            ),
        )?;
        let detected_language = run_whisper(
            &app,
            &job,
            &whisper,
            &model_path,
            &wav_path,
            &subtitle_prefix,
        )?;

        if !subtitle_path.exists() {
            return Err("whisper-cli 执行完成，但没有生成 .srt 文件".to_string());
        }

        ensure_not_cancelled(&job)?;
        emit_progress(&app, &job.job_id, "writing", "识别完成，正在写入字幕文件")?;
        app.emit_to(
            "main",
            "asr://completed",
            AsrCompletedPayload {
                job_id: job.job_id.clone(),
                subtitle_path: subtitle_path.display().to_string(),
                wav_path: wav_path.display().to_string(),
                model_path: model_path.display().to_string(),
                detected_language,
            },
        )
        .map_err(|error| format!("发送识别完成事件失败: {error}"))?;

        Ok(())
    })();

    if let Err(error) = result {
        let message = if job.cancel_requested.load(Ordering::SeqCst) {
            "当前识别任务已取消".to_string()
        } else {
            error
        };
        emit_failed(message);
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
    stage: &'static str,
    message: &str,
) -> Result<(), String> {
    app.emit_to(
        "main",
        "asr://progress",
        AsrProgressPayload {
            job_id: job_id.to_string(),
            stage,
            message: message.to_string(),
            percent: None,
        },
    )
    .map_err(|error| format!("发送进度事件失败: {error}"))
}

fn emit_progress_with_percent(
    app: &AppHandle,
    job_id: &str,
    stage: &'static str,
    message: &str,
    percent: f32,
) -> Result<(), String> {
    app.emit_to(
        "main",
        "asr://progress",
        AsrProgressPayload {
            job_id: job_id.to_string(),
            stage,
            message: message.to_string(),
            percent: Some(percent),
        },
    )
    .map_err(|error| format!("发送进度事件失败: {error}"))
}

fn ensure_not_cancelled(job: &AsrJobState) -> Result<(), String> {
    if job.cancel_requested.load(Ordering::SeqCst) {
        Err("当前识别任务已取消".to_string())
    } else {
        Ok(())
    }
}

fn wait_for_active_child(
    job: &AsrJobState,
    label: &str,
) -> Result<std::process::ExitStatus, String> {
    loop {
        let status = {
            let mut child_guard = job
                .active_child
                .lock()
                .map_err(|_| "识别进程句柄锁已损坏".to_string())?;
            let child = child_guard
                .as_mut()
                .ok_or_else(|| format!("未找到 {label} 进程句柄"))?;
            child
                .try_wait()
                .map_err(|error| format!("轮询 {label} 进程状态失败: {error}"))?
        };

        if let Some(status) = status {
            return Ok(status);
        }

        thread::sleep(Duration::from_millis(120));
    }
}

fn run_ffmpeg(
    job: &AsrJobState,
    target: &CommandTarget,
    input: &Path,
    output: &Path,
) -> Result<(), String> {
    // 限制 ffmpeg 为 1 线程 + taskpolicy -b / nice -n 19，与 whisper 策略一致
    let mut child = sidecar::build_nice_command(target)
        .args([
            "-y",
            "-threads",
            "1",
            "-v",
            "error",
            "-i",
            &input.display().to_string(),
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            &output.display().to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("执行 ffmpeg 失败: {error}"))?;

    let mut stderr = child.stderr.take();
    {
        let mut child_guard = job
            .active_child
            .lock()
            .map_err(|_| "识别进程句柄锁已损坏".to_string())?;
        *child_guard = Some(child);
    }

    let status = wait_for_active_child(job, "ffmpeg")?;

    {
        let mut child_guard = job
            .active_child
            .lock()
            .map_err(|_| "识别进程句柄锁已损坏".to_string())?;
        child_guard.take();
    }

    if status.success() {
        Ok(())
    } else {
        if job.cancel_requested.load(Ordering::SeqCst) {
            return Err("当前识别任务已取消".to_string());
        }

        let mut detail = String::new();
        if let Some(stderr_reader) = stderr.as_mut() {
            let _ = stderr_reader.read_to_string(&mut detail);
        }
        Err(format!("ffmpeg 转码失败: {}", detail.trim()))
    }
}

fn run_whisper(
    app: &AppHandle,
    job: &AsrJobState,
    target: &CommandTarget,
    model_path: &Path,
    wav_path: &Path,
    subtitle_prefix: &Path,
) -> Result<Option<String>, String> {
    // 限制 whisper-cli 线程数为 1：
    // 即使单线程 100% CPU 也只占一个核心，配合 taskpolicy -b (macOS) 或 nice -n 19 (Linux)
    // 可以确保 UI 始终流畅。识别速度会慢一些但用户体验远比"程序卡死"好。
    let whisper_threads = 1;

    // 计算 WAV 文件时长，用于解析 whisper 输出中的时间戳计算真实进度
    let total_duration_ms = wav_duration_ms(wav_path).unwrap_or(0);

    // 使用 nice -n 19 降低 CPU 调度优先级到最低，确保 UI 流畅
    // 使用 spawn() + stderr 解析替代 output()，实现实时进度回报
    let mut child = sidecar::build_nice_command(target)
        .args([
            "-m",
            &model_path.display().to_string(),
            "-f",
            &wav_path.display().to_string(),
            "-l",
            WHISPER_LANGUAGE_MODE,
            "-t",
            &whisper_threads.to_string(),
            "-osrt",
            "-of",
            &subtitle_prefix.display().to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动 whisper-cli 失败: {error}"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "未获取到 whisper-cli 错误输出".to_string())?;
    {
        let mut child_guard = job
            .active_child
            .lock()
            .map_err(|_| "识别进程句柄锁已损坏".to_string())?;
        *child_guard = Some(child);
    }

    // 实时解析 whisper stderr 输出，提取时间戳算识别进度
    let mut last_error_lines: Vec<String> = Vec::new();
    let reader = BufReader::new(stderr);
    let mut last_emit = Instant::now();
    let mut detected_language: Option<String> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if let Some(language) = parse_whisper_detected_language(&line) {
            detected_language = Some(language);
            continue;
        }

        // 尝试解析 whisper 时间戳行: [00:00:05.280 --> 00:00:10.560]
        if let Some(pct) = parse_whisper_progress(&line, total_duration_ms) {
            // 节流：最多 1 秒 1 次 IPC 事件，避免前端高频更新
            let now = Instant::now();
            if now.duration_since(last_emit).as_millis() >= 1000 {
                let _ = emit_progress_with_percent(
                    app,
                    &job.job_id,
                    "recognizing",
                    &format!("正在离线识别字幕… {:.0}%", pct),
                    pct as f32,
                );
                last_emit = now;
            }
        } else if !line.trim().is_empty() {
            // 保留最后几行非时间戳输出作为错误信息
            if last_error_lines.len() > 5 {
                last_error_lines.remove(0);
            }
            last_error_lines.push(line);
        }
    }

    let status = wait_for_active_child(job, "whisper-cli")?;

    {
        let mut child_guard = job
            .active_child
            .lock()
            .map_err(|_| "识别进程句柄锁已损坏".to_string())?;
        child_guard.take();
    }

    if status.success() {
        Ok(detected_language)
    } else {
        if job.cancel_requested.load(Ordering::SeqCst) {
            return Err("当前识别任务已取消".to_string());
        }

        let detail = if last_error_lines.is_empty() {
            format_exit_code_detail(status.code())
        } else {
            last_error_lines.join("\n")
        };
        Err(format!("whisper-cli 识别失败: {}", detail.trim()))
    }
}

fn parse_whisper_detected_language(line: &str) -> Option<String> {
    let marker = "auto-detected language:";
    let lowered = line.to_ascii_lowercase();
    let start = lowered.find(marker)? + marker.len();
    let language = line[start..]
        .trim()
        .split_whitespace()
        .next()?
        .trim_matches(|char: char| !char.is_ascii_alphanumeric() && char != '-')
        .to_ascii_lowercase();

    if language.is_empty() {
        None
    } else {
        Some(language)
    }
}

/// 从 whisper stderr 行解析识别进度
/// 格式: [00:00:05.280 --> 00:00:10.560]  some text...
fn format_exit_code_detail(code: Option<i32>) -> String {
    #[cfg(windows)]
    {
        if code == Some(-1073741515) {
            return "exit code: Some(-1073741515) (Windows DLL load failed; whisper-cli could not find its bundled DLLs)".to_string();
        }
    }

    format!("exit code: {code:?}")
}

fn parse_whisper_progress(line: &str, total_duration_ms: u64) -> Option<f64> {
    let line = line.trim();
    if !line.starts_with('[') {
        return None;
    }
    let arrow = line.find(" --> ")?;
    let bracket_end = line[arrow..].find(']').map(|i| i + arrow)?;

    let end_str = &line[arrow + 4..bracket_end];
    let end_ms = parse_timestamp_ms(end_str)?;

    if total_duration_ms == 0 {
        return None;
    }
    Some((end_ms as f64 / total_duration_ms as f64 * 100.0).min(100.0))
}

/// 解析时间戳 "00:00:05.280" → 毫秒
fn parse_timestamp_ms(ts: &str) -> Option<u64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: u64 = parts[0].parse().ok()?;
    let minutes: u64 = parts[1].parse().ok()?;
    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    if sec_parts.len() != 2 {
        return None;
    }
    let seconds: u64 = sec_parts[0].parse().ok()?;
    let millis: u64 = sec_parts[1].parse().ok()?;
    Some(hours * 3600_000 + minutes * 60_000 + seconds * 1000 + millis)
}

/// 估算 16kHz/16bit/mono WAV 文件的时长（毫秒）
fn wav_duration_ms(wav_path: &Path) -> Result<u64, String> {
    let meta = fs::metadata(wav_path).map_err(|e| format!("读取 WAV 文件信息失败: {e}"))?;
    // 16kHz × 16bit × mono = 32000 bytes/sec, WAV header 约 44 bytes
    let data_bytes = meta.len().saturating_sub(44);
    Ok((data_bytes as f64 / 32000.0 * 1000.0) as u64)
}

fn is_whisper_ready_wav(path: &Path) -> Result<bool, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("wav") {
        return Ok(false);
    }

    let file = fs::File::open(path).map_err(|error| format!("读取 WAV 文件失败: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut header = [0u8; 12];
    reader
        .read_exact(&mut header)
        .map_err(|error| format!("读取 WAV 头失败: {error}"))?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Ok(false);
    }

    const MAX_FMT_CHUNK_SIZE: usize = 4096;

    loop {
        let mut chunk_header = [0u8; 8];
        match reader.read_exact(&mut chunk_header) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(error) => return Err(format!("读取 WAV chunk 失败: {error}")),
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as usize;

        if chunk_id == b"fmt " {
            if chunk_size < 16 || chunk_size > MAX_FMT_CHUNK_SIZE {
                return Ok(false);
            }

            let mut fmt_chunk = vec![0u8; chunk_size];
            reader
                .read_exact(&mut fmt_chunk)
                .map_err(|error| format!("读取 WAV fmt chunk 失败: {error}"))?;

            let audio_format = u16::from_le_bytes([fmt_chunk[0], fmt_chunk[1]]);
            let channels = u16::from_le_bytes([fmt_chunk[2], fmt_chunk[3]]);
            let sample_rate =
                u32::from_le_bytes([fmt_chunk[4], fmt_chunk[5], fmt_chunk[6], fmt_chunk[7]]);
            let bits_per_sample = u16::from_le_bytes([fmt_chunk[14], fmt_chunk[15]]);

            return Ok(audio_format == 1
                && channels == 1
                && sample_rate == 16_000
                && bits_per_sample == 16);
        }

        let skipped = io::copy(
            &mut reader.by_ref().take(chunk_size as u64),
            &mut io::sink(),
        )
        .map_err(|error| format!("跳过 WAV chunk 失败: {error}"))?;
        if skipped != chunk_size as u64 {
            return Ok(false);
        }

        if chunk_size % 2 == 1 {
            let mut padding = [0u8; 1];
            match reader.read_exact(&mut padding) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
                Err(error) => return Err(format!("读取 WAV 对齐字节失败: {error}")),
            }
        }
    }
}

fn locate_ffmpeg(app: &AppHandle) -> Result<CommandTarget, String> {
    sidecar::locate_executable(app, "FFMPEG_BIN", &["ffmpeg"])
}

fn locate_whisper_cli(app: &AppHandle) -> Result<CommandTarget, String> {
    sidecar::locate_executable(
        app,
        "WHISPER_CLI_BIN",
        &["whisper-cli", "whisper_cpp", "whisper"],
    )
}

fn locate_whisper_model(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("WHISPER_MODEL_PATH") {
        let model_path = PathBuf::from(path);
        if model_path.exists() {
            ensure_multilingual_whisper_model(&model_path)?;
            return Ok(model_path);
        }
    }

    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    let candidates = [
        model::resolve_default_model_path(app)?,
        current_dir.join("models/ggml-base.bin"),
        current_dir.join("src-tauri/models/ggml-base.bin"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            ensure_multilingual_whisper_model(&candidate)?;
            return Ok(candidate);
        }
    }

    Err(
        "未找到 Whisper 模型文件。请把模型放到 ./models/ggml-base.bin，或设置环境变量 WHISPER_MODEL_PATH。"
            .to_string(),
    )
}

fn ensure_multilingual_whisper_model(model_path: &Path) -> Result<(), String> {
    let file_name = model_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let is_english_only = file_name.contains(".en.") || file_name.contains(".en-");
    if is_english_only {
        return Err(format!(
            "当前 Whisper 模型 {} 是英文专用模型，无法稳定识别非英语音频。请切换到 tiny、base、small、medium 或 large-v3-turbo 等多语言模型。",
            model_path.display()
        ));
    }

    Ok(())
}

fn ensure_dir(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join(relative);

    fs::create_dir_all(&path).map_err(|error| format!("创建目录失败: {error}"))?;
    Ok(path)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("subtitle")
        .to_string()
}

fn generate_job_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let rand: u32 = (millis as u32).wrapping_mul(2654435761);
    format!("asr-{millis}-{rand:08x}")
}
