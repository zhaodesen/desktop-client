use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::model;
use crate::sidecar::{self, CommandTarget};

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrCompletedPayload {
    pub job_id: String,
    pub subtitle_path: String,
    pub wav_path: String,
    pub model_path: String,
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
    active_job: Arc<Mutex<Option<String>>>,
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
        let spawned_job_id = job_id.clone();
        *guard = Some(job_id.clone());

        let app_handle = app.clone();
        let active_job_ref = active_job.clone();
        thread::spawn(move || {
            run_pipeline(app_handle, active_job_ref, spawned_job_id, audio_path);
        });

        return Ok(StartAsrJobOutput { job_id });
    }
}

fn run_pipeline(
    app: AppHandle,
    active_job: Arc<Mutex<Option<String>>>,
    job_id: String,
    audio_path: PathBuf,
) {
    let emit_failed = |message: String| {
        let _ = app.emit_to(
            "main",
            "asr://failed",
            AsrFailedPayload {
                job_id: job_id.clone(),
                code: "asr_pipeline_failed",
                message,
            },
        );
    };

    let result = (|| -> Result<(), String> {
        app.emit_to(
            "main",
            "asr://started",
            AsrStartedPayload {
                job_id: job_id.clone(),
                audio_path: audio_path.display().to_string(),
            },
        )
        .map_err(|error| format!("发送任务开始事件失败: {error}"))?;

        emit_progress(
            &app,
            &job_id,
            "preparing",
            "正在检查 ffmpeg、whisper-cli 和模型文件",
        )?;

        let ffmpeg = locate_ffmpeg(&app)?;
        let whisper = locate_whisper_cli(&app)?;
        let model_path = locate_whisper_model(&app)?;

        let cache_audio_dir = ensure_dir(&app, "cache/audio")?;
        let subtitle_dir = ensure_dir(&app, "subtitles")?;
        let wav_path = cache_audio_dir.join(format!("{job_id}.wav"));
        let subtitle_prefix = subtitle_dir.join(format!("{}-{job_id}", file_stem(&audio_path)));
        let subtitle_path = subtitle_prefix.with_extension("srt");

        emit_progress(
            &app,
            &job_id,
            "preparing",
            &format!("正在使用 {} 进行音频标准化", ffmpeg.display()),
        )?;
        run_ffmpeg(&ffmpeg, &audio_path, &wav_path)?;

        emit_progress(
            &app,
            &job_id,
            "recognizing",
            &format!(
                "正在使用 {} 和模型 {} 进行离线识别",
                whisper.display(),
                model_path.display()
            ),
        )?;
        run_whisper(&whisper, &model_path, &wav_path, &subtitle_prefix)?;

        if !subtitle_path.exists() {
            return Err("whisper-cli 执行完成，但没有生成 .srt 文件".to_string());
        }

        emit_progress(&app, &job_id, "writing", "识别完成，正在写入字幕文件")?;
        app.emit_to(
            "main",
            "asr://completed",
            AsrCompletedPayload {
                job_id: job_id.clone(),
                subtitle_path: subtitle_path.display().to_string(),
                wav_path: wav_path.display().to_string(),
                model_path: model_path.display().to_string(),
            },
        )
        .map_err(|error| format!("发送识别完成事件失败: {error}"))?;

        Ok(())
    })();

    if let Err(error) = result {
        emit_failed(error);
    }

    if let Ok(mut guard) = active_job.lock() {
        *guard = None;
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
        },
    )
    .map_err(|error| format!("发送进度事件失败: {error}"))
}

fn run_ffmpeg(target: &CommandTarget, input: &Path, output: &Path) -> Result<(), String> {
    let output_result = sidecar::build_command(target)
        .args([
            "-y",
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
        .output()
        .map_err(|error| format!("执行 ffmpeg 失败: {error}"))?;

    if output_result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg 转码失败: {}",
            String::from_utf8_lossy(&output_result.stderr).trim()
        ))
    }
}

fn run_whisper(
    target: &CommandTarget,
    model_path: &Path,
    wav_path: &Path,
    subtitle_prefix: &Path,
) -> Result<(), String> {
    let output_result = sidecar::build_command(target)
        .args([
            "-m",
            &model_path.display().to_string(),
            "-f",
            &wav_path.display().to_string(),
            "-osrt",
            "-of",
            &subtitle_prefix.display().to_string(),
        ])
        .output()
        .map_err(|error| format!("执行 whisper-cli 失败: {error}"))?;

    if output_result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "whisper-cli 识别失败: {}",
            String::from_utf8_lossy(&output_result.stderr).trim()
        ))
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
            return Ok(candidate);
        }
    }

    Err(
        "未找到 Whisper 模型文件。请把模型放到 ./models/ggml-base.bin，或设置环境变量 WHISPER_MODEL_PATH。"
            .to_string(),
    )
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
