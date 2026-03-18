use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

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

struct CommandTarget(PathBuf);

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
            fs::copy(&source_path, &target).map_err(|error| format!("复制音频文件失败: {error}"))?;
            target
        }
        MediaSourceKind::Video => {
            let ffmpeg = locate_ffmpeg(app)?;
            let target = media_dir.join(format!("{id}.wav"));
            extract_audio_with_ffmpeg(&ffmpeg, &source_path, &target)?;
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

fn locate_ffmpeg(app: &AppHandle) -> Result<CommandTarget, String> {
    if let Ok(value) = env::var("FFMPEG_BIN") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Ok(CommandTarget(path));
        }
    }

    for candidate in resolve_local_candidates(app, &["ffmpeg"])? {
        if candidate.exists() {
            return Ok(CommandTarget(candidate));
        }
    }

    Err(
        "未找到 ffmpeg 可执行文件。请把二进制放到 src-tauri/binaries 并通过 externalBin 打包，或通过环境变量 FFMPEG_BIN 指定绝对路径。"
            .to_string(),
    )
}

fn extract_audio_with_ffmpeg(
    target: &CommandTarget,
    source_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let output = Command::new(&target.0)
        .args([
            "-y",
            "-i",
            &source_path.display().to_string(),
            "-vn",
            "-ar",
            "44100",
            "-ac",
            "2",
            "-c:a",
            "pcm_s16le",
            &output_path.display().to_string(),
        ])
        .output()
        .map_err(|error| format!("执行 ffmpeg 失败: {error}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "视频转音频失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn resolve_local_candidates(app: &AppHandle, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;

    for name in names {
        let binary_name = with_target_triple(name);
        if let Ok(resource_path) = app
            .path()
            .resolve(format!("binaries/{binary_name}"), BaseDirectory::Resource)
        {
            candidates.push(resource_path);
        }
        if let Ok(resource_path) = app.path().resolve(
            format!("binaries/{}", with_exe_suffix(name)),
            BaseDirectory::Resource,
        ) {
            candidates.push(resource_path);
        }
        candidates.push(current_dir.join("src-tauri/binaries").join(&binary_name));
        candidates.push(
            current_dir
                .join("src-tauri/binaries")
                .join(with_exe_suffix(name)),
        );
        candidates.push(current_dir.join("bin").join(with_exe_suffix(name)));
    }

    Ok(candidates)
}

fn with_exe_suffix(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn with_target_triple(name: &str) -> String {
    let triple = option_env!("TAURI_ENV_TARGET_TRIPLE").unwrap_or("");
    if triple.is_empty() {
        return with_exe_suffix(name);
    }

    if cfg!(windows) {
        format!("{name}-{triple}.exe")
    } else {
        format!("{name}-{triple}")
    }
}

fn generate_id(prefix: &str) -> String {
    format!("{prefix}-{}", now_millis())
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
