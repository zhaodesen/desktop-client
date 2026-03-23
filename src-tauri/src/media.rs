use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

use crate::sidecar::{self, CommandTarget};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaSourceKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub original_file_name: String,
    pub source_kind: MediaSourceKind,
    pub audio_path: String,
    pub subtitle_path: Option<String>,
    pub imported_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHistoryItem {
    pub media_id: String,
    pub title: String,
    pub audio_path: String,
    pub subtitle_path: Option<String>,
    pub played_at: u64,
    pub play_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryState {
    pub media_items: Vec<MediaItem>,
    pub playback_history: Vec<PlaybackHistoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaInput {
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMediaProgressPayload {
    pub stage: &'static str,
    pub message: String,
    pub percent: Option<f32>,
}


pub fn get_library_state(app: &AppHandle) -> Result<LibraryState, String> {
    load_library_state(app)
}

pub fn import_media(app: &AppHandle, input: ImportMediaInput) -> Result<MediaItem, String> {
    let source_path = PathBuf::from(&input.source_path);
    if !source_path.exists() {
        return Err("导入文件不存在".to_string());
    }

    let mut state = load_library_state(app)?;
    let id = generate_id("media");
    let original_file_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("media")
        .to_string();
    let title = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名素材")
        .to_string();

    let source_kind = if is_video_file(&source_path) {
        MediaSourceKind::Video
    } else {
        MediaSourceKind::Audio
    };

    let media_dir = ensure_media_dir(app)?;
    let audio_path = match source_kind {
        MediaSourceKind::Audio => {
            let extension = source_path
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("mp3");
            let target = media_dir.join(format!("{id}.{extension}"));
            emit_import_progress(app, "copying", "正在复制音频文件…", Some(0.0))?;
            copy_file_with_progress(app, &source_path, &target)?;
            target
        }
        MediaSourceKind::Video => {
            let ffmpeg = sidecar::locate_executable(app, "FFMPEG_BIN", &["ffmpeg"])?;
            let target = media_dir.join(format!("{id}.wav"));
            emit_import_progress(app, "extracting", "正在从视频提取音频…", Some(0.0))?;
            extract_audio_with_ffmpeg(app, &ffmpeg, &source_path, &target)?;
            target
        }
    };

    let item = MediaItem {
        id,
        title,
        original_file_name,
        source_kind,
        audio_path: audio_path.display().to_string(),
        subtitle_path: None,
        imported_at: now_millis(),
    };

    emit_import_progress(app, "registering", "正在写入素材索引…", Some(100.0))?;
    state.media_items.insert(0, item.clone());
    save_library_state(app, &state)?;
    Ok(item)
}

pub fn delete_media(app: &AppHandle, media_id: &str) -> Result<(), String> {
    let mut state = load_library_state(app)?;
    let Some(index) = state.media_items.iter().position(|item| item.id == media_id) else {
        return Ok(());
    };

    let item = state.media_items.remove(index);
    remove_if_exists(Path::new(&item.audio_path))?;
    if let Some(path) = &item.subtitle_path {
        remove_if_exists(Path::new(path))?;
    }

    state.playback_history.retain(|entry| entry.media_id != media_id);
    save_library_state(app, &state)
}

pub fn clear_subtitle_references(app: &AppHandle) -> Result<(), String> {
    let mut state = load_library_state(app)?;
    for item in &mut state.media_items {
        item.subtitle_path = None;
    }
    for entry in &mut state.playback_history {
        entry.subtitle_path = None;
    }
    save_library_state(app, &state)
}

pub fn update_media_subtitle(
    app: &AppHandle,
    media_id: &str,
    subtitle_path: &str,
) -> Result<MediaItem, String> {
    let mut state = load_library_state(app)?;
    let mut updated: Option<MediaItem> = None;

    for item in &mut state.media_items {
        if item.id == media_id {
            item.subtitle_path = Some(subtitle_path.to_string());
            updated = Some(item.clone());
        }
    }

    for entry in &mut state.playback_history {
        if entry.media_id == media_id {
            entry.subtitle_path = Some(subtitle_path.to_string());
        }
    }

    let item = updated.ok_or_else(|| "未找到对应素材".to_string())?;
    save_library_state(app, &state)?;
    Ok(item)
}

pub fn record_playback(app: &AppHandle, media_id: &str) -> Result<PlaybackHistoryItem, String> {
    let mut state = load_library_state(app)?;
    let item = state
        .media_items
        .iter()
        .find(|entry| entry.id == media_id)
        .cloned()
        .ok_or_else(|| "未找到对应素材".to_string())?;

    let now = now_millis();
    if let Some(index) = state
        .playback_history
        .iter()
        .position(|entry| entry.media_id == media_id)
    {
        let mut entry = state.playback_history.remove(index);
        entry.played_at = now;
        entry.play_count += 1;
        entry.subtitle_path = item.subtitle_path.clone();
        state.playback_history.insert(0, entry.clone());
        save_library_state(app, &state)?;
        return Ok(entry);
    }

    let entry = PlaybackHistoryItem {
        media_id: item.id,
        title: item.title,
        audio_path: item.audio_path,
        subtitle_path: item.subtitle_path,
        played_at: now,
        play_count: 1,
    };

    state.playback_history.insert(0, entry.clone());
    save_library_state(app, &state)?;
    Ok(entry)
}

pub fn remove_playback_item(app: &AppHandle, media_id: &str) -> Result<(), String> {
    let mut state = load_library_state(app)?;
    let original_len = state.playback_history.len();
    state.playback_history.retain(|entry| entry.media_id != media_id);
    if state.playback_history.len() == original_len {
        return Ok(());
    }
    save_library_state(app, &state)
}

fn load_library_state(app: &AppHandle) -> Result<LibraryState, String> {
    let path = library_file_path(app)?;
    if !path.exists() {
        let state = LibraryState::default();
        save_library_state(app, &state)?;
        return Ok(state);
    }

    let content = fs::read_to_string(&path).map_err(|error| format!("读取素材库失败: {error}"))?;
    serde_json::from_str(&content).map_err(|error| format!("解析素材库失败: {error}"))
}

fn save_library_state(app: &AppHandle, state: &LibraryState) -> Result<(), String> {
    let path = library_file_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建素材库目录失败: {error}"))?;
    }
    let content =
        serde_json::to_string_pretty(state).map_err(|error| format!("序列化素材库失败: {error}"))?;
    fs::write(&path, content).map_err(|error| format!("写入素材库失败: {error}"))
}

fn library_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("library.json"))
}

fn ensure_media_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("media");
    fs::create_dir_all(&dir).map_err(|error| format!("创建媒体目录失败: {error}"))?;
    Ok(dir)
}

fn is_video_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi"
    )
}

fn emit_import_progress(
    app: &AppHandle,
    stage: &'static str,
    message: &str,
    percent: Option<f32>,
) -> Result<(), String> {
    app.emit_to(
        "main",
        "import://progress",
        ImportMediaProgressPayload {
            stage,
            message: message.to_string(),
            percent,
        },
    )
    .map_err(|error| format!("发送导入进度事件失败: {error}"))
}

fn copy_file_with_progress(app: &AppHandle, source_path: &Path, target_path: &Path) -> Result<(), String> {
    let total_bytes = fs::metadata(source_path)
        .map_err(|error| format!("读取源文件信息失败: {error}"))?
        .len();
    let mut source =
        File::open(source_path).map_err(|error| format!("打开源文件失败: {error}"))?;
    let mut target =
        File::create(target_path).map_err(|error| format!("创建目标文件失败: {error}"))?;

    let mut copied_bytes = 0u64;
    let mut last_emitted = 0f32;
    let mut buffer = vec![0u8; 1024 * 512];

    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("读取源文件失败: {error}"))?;
        if read == 0 {
            break;
        }

        target
            .write_all(&buffer[..read])
            .map_err(|error| format!("写入目标文件失败: {error}"))?;
        copied_bytes += read as u64;

        if total_bytes > 0 {
            let percent = ((copied_bytes as f64 / total_bytes as f64) * 100.0).min(100.0) as f32;
            if percent >= last_emitted + 5.0 || percent >= 100.0 {
                emit_import_progress(
                    app,
                    "copying",
                    &format!("正在复制音频文件… {:.0}%", percent),
                    Some(percent),
                )?;
                last_emitted = percent;
            }
        }
    }

    if total_bytes == 0 {
        emit_import_progress(app, "copying", "正在复制音频文件…", None)?;
    } else if last_emitted < 100.0 {
        emit_import_progress(app, "copying", "正在复制音频文件… 100%", Some(100.0))?;
    }

    Ok(())
}

fn extract_audio_with_ffmpeg(
    app: &AppHandle,
    target: &CommandTarget,
    source_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    // 限制 ffmpeg 为 1 线程 + taskpolicy -b (macOS) / nice -n 19 (Linux)
    // 确保视频提取音频时 UI 不卡
    let mut child = sidecar::build_nice_command(target)
        .args([
            "-y",
            "-threads",
            "1",
            "-i",
            &source_path.display().to_string(),
            "-vn",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            "-progress",
            "pipe:2",
            "-nostats",
            &output_path.display().to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("执行 ffmpeg 失败: {error}"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "未获取到 ffmpeg 进度输出".to_string())?;
    let reader = BufReader::new(stderr);
    let mut duration_ms = 0u64;
    let mut last_emitted = 0f32;
    let mut last_error_lines: Vec<String> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };

        if duration_ms == 0 {
            if let Some(parsed_duration) = parse_ffmpeg_duration_ms(&line) {
                duration_ms = parsed_duration;
            }
        }

        if let Some(current_ms) = parse_ffmpeg_progress_ms(&line) {
            if duration_ms > 0 {
                let percent = ((current_ms as f64 / duration_ms as f64) * 100.0).min(100.0) as f32;
                if percent >= last_emitted + 5.0 || percent >= 100.0 {
                    emit_import_progress(
                        app,
                        "extracting",
                        &format!("正在从视频提取音频… {:.0}%", percent),
                        Some(percent),
                    )?;
                    last_emitted = percent;
                }
            } else if last_emitted == 0.0 {
                emit_import_progress(app, "extracting", "正在从视频提取音频…", None)?;
                last_emitted = 1.0;
            }
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("ffmpeg version")
            || trimmed.starts_with("Input #")
            || trimmed.starts_with("Stream #")
            || trimmed.starts_with("Metadata:")
        {
            continue;
        }
        if last_error_lines.len() >= 6 {
            last_error_lines.remove(0);
        }
        last_error_lines.push(trimmed.to_string());
    }

    let status = child
        .wait()
        .map_err(|error| format!("等待 ffmpeg 结束失败: {error}"))?;

    if status.success() {
        if last_emitted < 100.0 {
            emit_import_progress(app, "extracting", "正在从视频提取音频… 100%", Some(100.0))?;
        }
        Ok(())
    } else {
        let detail = if last_error_lines.is_empty() {
            format!("exit code: {:?}", status.code())
        } else {
            last_error_lines.join("\n")
        };
        Err(format!(
            "视频转音频失败: {}",
            detail.trim()
        ))
    }
}

fn parse_ffmpeg_duration_ms(line: &str) -> Option<u64> {
    let start = line.find("Duration: ")? + "Duration: ".len();
    let end = line[start..]
        .find(',')
        .map(|index| start + index)
        .unwrap_or(line.len());
    parse_media_timestamp_ms(&line[start..end])
}

fn parse_ffmpeg_progress_ms(line: &str) -> Option<u64> {
    let trimmed = line.trim();
    if let Some(value) = trimmed.strip_prefix("out_time_ms=") {
        return value.parse::<u64>().ok().map(|micros| micros / 1000);
    }
    if let Some(value) = trimmed.strip_prefix("out_time_us=") {
        return value.parse::<u64>().ok().map(|micros| micros / 1000);
    }
    if let Some(value) = trimmed.strip_prefix("out_time=") {
        return parse_media_timestamp_ms(value);
    }
    None
}

fn parse_media_timestamp_ms(value: &str) -> Option<u64> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: u64 = parts[0].parse().ok()?;
    let minutes: u64 = parts[1].parse().ok()?;
    let seconds_part = parts[2];
    let (seconds_text, fraction_text) = seconds_part
        .split_once('.')
        .or_else(|| seconds_part.split_once(','))
        .unwrap_or((seconds_part, "0"));
    let seconds: u64 = seconds_text.parse().ok()?;

    let normalized_fraction = match fraction_text.len() {
        0 => "000".to_string(),
        1 => format!("{fraction_text}00"),
        2 => format!("{fraction_text}0"),
        _ => fraction_text[..3].to_string(),
    };
    let millis: u64 = normalized_fraction.parse().ok()?;

    Some(hours * 3600_000 + minutes * 60_000 + seconds * 1000 + millis)
}

fn generate_id(prefix: &str) -> String {
    let millis = now_millis();
    let rand: u32 = (millis as u32).wrapping_mul(2654435761);
    format!("{prefix}-{millis}-{rand:08x}")
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("删除文件失败: {error}"))?;
    }
    Ok(())
}
