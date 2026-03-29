use serde::{Deserialize, Serialize};
use std::{
    fs,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use std::env;

use crate::sidecar::{self, CommandTarget};
use crate::state::ExternalProcessState;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDebugProbe {
    pub requested_path: String,
    pub exists: bool,
    pub is_file: bool,
    pub file_size_bytes: Option<u64>,
    pub extension: Option<String>,
    pub canonical_path: Option<String>,
    pub app_data_dir: String,
    pub media_dir: String,
    pub library_file_path: String,
    pub inside_app_data: bool,
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
    let source_path = PathBuf::from(input.source_path);
    import_media_from_source_path(app, &source_path)
}

pub fn import_online_media(
    app: &AppHandle,
    url: &str,
    active_import: Arc<Mutex<Option<ExternalProcessState>>>,
) -> Result<MediaItem, String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err("请输入在线视频地址".to_string());
    }

    let downloaded_path = download_online_media(app, trimmed_url, active_import)?;
    import_media_from_source_path(app, &downloaded_path)
}

fn import_media_from_source_path(app: &AppHandle, source_path: &Path) -> Result<MediaItem, String> {
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

    let source_kind = if is_video_file(source_path) {
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
            copy_file_with_progress(app, source_path, &target)?;
            target
        }
        MediaSourceKind::Video => {
            let ffmpeg = sidecar::locate_executable(app, "FFMPEG_BIN", &["ffmpeg"])?;
            let target = media_dir.join(format!("{id}.wav"));
            emit_import_progress(app, "extracting", "正在从视频提取音频…", Some(0.0))?;
            extract_audio_with_ffmpeg(app, &ffmpeg, source_path, &target)?;
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
    let Some(index) = state
        .media_items
        .iter()
        .position(|item| item.id == media_id)
    else {
        return Ok(());
    };

    let item = state.media_items.remove(index);
    remove_if_exists(Path::new(&item.audio_path))?;
    if let Some(path) = &item.subtitle_path {
        remove_if_exists(Path::new(path))?;
    }

    state
        .playback_history
        .retain(|entry| entry.media_id != media_id);
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
    if let Some(entry) = state
        .playback_history
        .iter_mut()
        .find(|entry| entry.media_id == media_id)
    {
        entry.played_at = now;
        entry.play_count += 1;
        entry.subtitle_path = item.subtitle_path.clone();
        let entry = entry.clone();
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

    state.playback_history.push(entry.clone());
    save_library_state(app, &state)?;
    Ok(entry)
}

pub fn remove_playback_item(app: &AppHandle, media_id: &str) -> Result<(), String> {
    let mut state = load_library_state(app)?;
    let original_len = state.playback_history.len();
    state
        .playback_history
        .retain(|entry| entry.media_id != media_id);
    if state.playback_history.len() == original_len {
        return Ok(());
    }
    save_library_state(app, &state)
}

pub fn probe_media_path(app: &AppHandle, path: &str) -> Result<MediaDebugProbe, String> {
    let requested_path = PathBuf::from(path);
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?;
    let media_dir = app_data_dir.join("media");
    let library_file_path = app_data_dir.join("library.json");

    let metadata = fs::metadata(&requested_path).ok();
    let canonical_path = fs::canonicalize(&requested_path).ok();
    let inside_app_data = canonical_path
        .as_ref()
        .map(|resolved| resolved.starts_with(&app_data_dir))
        .unwrap_or_else(|| requested_path.starts_with(&app_data_dir));

    Ok(MediaDebugProbe {
        requested_path: requested_path.display().to_string(),
        exists: requested_path.exists(),
        is_file: metadata.as_ref().is_some_and(|value| value.is_file()),
        file_size_bytes: metadata.map(|value| value.len()),
        extension: requested_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string()),
        canonical_path: canonical_path.map(|value| value.display().to_string()),
        app_data_dir: app_data_dir.display().to_string(),
        media_dir: media_dir.display().to_string(),
        library_file_path: library_file_path.display().to_string(),
        inside_app_data,
    })
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
    let content = serde_json::to_string_pretty(state)
        .map_err(|error| format!("序列化素材库失败: {error}"))?;
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

fn ensure_online_download_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base_dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir().map(|path| path.join("Downloads")))
        .map_err(|error| format!("读取系统下载目录失败: {error}"))?;
    let dir = base_dir.join("muyu").join("在线视频");
    fs::create_dir_all(&dir).map_err(|error| format!("创建在线视频目录失败: {error}"))?;
    Ok(dir)
}

#[derive(Debug)]
struct YtDlpPipeLine {
    is_stdout: bool,
    line: String,
}

#[derive(Debug, Clone)]
struct YtDlpAttempt {
    label: String,
    args: Vec<String>,
}

const BILIBILI_REFERER: &str = "https://www.bilibili.com/";
const BILIBILI_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

fn download_online_media(
    app: &AppHandle,
    url: &str,
    active_import: Arc<Mutex<Option<ExternalProcessState>>>,
) -> Result<PathBuf, String> {
    emit_import_progress(app, "downloading", "正在准备在线视频下载…", Some(0.0))?;

    let yt_dlp = sidecar::locate_executable(app, "YT_DLP_BIN", &["yt-dlp"])?;
    let download_dir = ensure_online_download_dir(app)?;
    let output_template = download_dir.join("%(title).160B [%(id)s].%(ext)s");
    let output_template_text = output_template.display().to_string();
    let attempts = build_yt_dlp_attempts(url, &output_template_text);
    let total_attempts = attempts.len();
    let mut last_error = None;

    for (index, attempt) in attempts.into_iter().enumerate() {
        if index > 0 {
            let retry_message = format!(
                "首次下载失败，正在重试（{}/{}）：{}…",
                index + 1,
                total_attempts,
                attempt.label
            );
            emit_import_progress(app, "downloading", &retry_message, Some(0.0))?;
        }

        match run_yt_dlp_download(app, &yt_dlp, &attempt.args, active_import.clone()) {
            Ok(path) => {
                emit_import_progress(
                    app,
                    "downloading",
                    "在线视频下载完成，准备导入…",
                    Some(100.0),
                )?;
                return Ok(path);
            }
            Err(error) => {
                last_error = Some(match last_error {
                    Some(previous) => select_preferred_download_error(previous, error),
                    None => error,
                });
            }
        }
    }

    let fallback_hint = if is_bilibili_url(url) {
        "。如仍失败，请先在本机浏览器登录 B 站后重试，应用会自动尝试读取浏览器 Cookie。"
    } else {
        ""
    };

    Err(format!(
        "{}{}",
        last_error.unwrap_or_else(|| "在线视频下载失败: 未知错误".to_string()),
        fallback_hint
    ))
}

fn run_yt_dlp_download(
    app: &AppHandle,
    yt_dlp: &CommandTarget,
    args: &[String],
    active_import: Arc<Mutex<Option<ExternalProcessState>>>,
) -> Result<PathBuf, String> {
    let mut child = sidecar::build_nice_command(yt_dlp)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动 yt-dlp 失败: {error}"))?;
    let pid = child.id();
    if let Ok(mut guard) = active_import.lock() {
        *guard = Some(ExternalProcessState::new("在线视频下载", pid));
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "未获取到 yt-dlp 标准输出".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "未获取到 yt-dlp 标准错误输出".to_string())?;

    let (tx, rx) = mpsc::channel::<YtDlpPipeLine>();
    let stdout_tx = tx.clone();
    thread::spawn(move || read_yt_dlp_pipe(stdout, true, stdout_tx));
    let stderr_tx = tx.clone();
    thread::spawn(move || read_yt_dlp_pipe(stderr, false, stderr_tx));
    drop(tx);

    let mut downloaded_path: Option<PathBuf> = None;
    let mut last_percent = 0f32;
    let mut last_message = "正在下载在线视频…".to_string();

    for output in rx {
        let line = output.line.trim();
        if line.is_empty() {
            continue;
        }

        if output.is_stdout {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                downloaded_path = Some(path);
                last_message = "视频下载完成，正在导入素材…".to_string();
                continue;
            }
        }

        if let Some(percent) = parse_yt_dlp_percent(line) {
            last_percent = percent.max(last_percent);
            last_message = format!("正在下载在线视频… {:.0}%", percent);
            emit_import_progress(app, "downloading", &last_message, Some(percent))?;
            continue;
        }

        if line.contains("Merging formats") || line.contains("Fixing") {
            last_percent = last_percent.max(96.0);
            last_message = "正在合并音视频流…".to_string();
            emit_import_progress(app, "downloading", &last_message, Some(last_percent))?;
            continue;
        }

        if line.starts_with("ERROR:") {
            last_message = line.trim_start_matches("ERROR:").trim().to_string();
            continue;
        }

        if !output.is_stdout
            && (line.contains("HTTP Error")
                || line.contains("unable to download")
                || line.contains("Unable to download")
                || line.contains("403")
                || line.contains("login"))
        {
            last_message = line.to_string();
        }
    }

    let result = (|| -> Result<PathBuf, String> {
        let status = child
            .wait()
            .map_err(|error| format!("等待 yt-dlp 结束失败: {error}"))?;

        if !status.success() {
            return Err(format!("在线视频下载失败: {}", last_message.trim()));
        }

        let Some(downloaded_path) = downloaded_path else {
            return Err("在线视频下载完成，但未拿到输出文件路径".to_string());
        };

        if !downloaded_path.exists() {
            return Err("在线视频下载完成，但目标文件不存在".to_string());
        }

        Ok(downloaded_path)
    })();

    if let Ok(mut guard) = active_import.lock() {
        if guard.as_ref().map(|task| task.pid == pid).unwrap_or(false) {
            *guard = None;
        }
    }

    result
}

fn build_yt_dlp_attempts(url: &str, output_template: &str) -> Vec<YtDlpAttempt> {
    let mut attempts = vec![YtDlpAttempt {
        label: "默认参数".to_string(),
        args: build_yt_dlp_args(url, output_template, &[]),
    }];

    if !is_bilibili_url(url) {
        return attempts;
    }

    let headers = bilibili_request_args();
    attempts.push(YtDlpAttempt {
        label: "补充 B 站请求头".to_string(),
        args: build_yt_dlp_args(url, output_template, &headers),
    });

    for browser in bilibili_cookie_browsers() {
        let mut extra_args = headers.clone();
        extra_args.push("--cookies-from-browser".to_string());
        extra_args.push(browser.to_string());
        attempts.push(YtDlpAttempt {
            label: format!("读取 {browser} Cookie"),
            args: build_yt_dlp_args(url, output_template, &extra_args),
        });
    }

    attempts
}

fn build_yt_dlp_args(url: &str, output_template: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec![
        "--no-playlist".to_string(),
        "--newline".to_string(),
        "--progress".to_string(),
        "--merge-output-format".to_string(),
        "mp4".to_string(),
        "--format".to_string(),
        "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b".to_string(),
        "--output".to_string(),
        output_template.to_string(),
        "--print".to_string(),
        "after_move:filepath".to_string(),
    ];
    args.extend(extra_args.iter().cloned());
    args.push(url.to_string());
    args
}

fn bilibili_request_args() -> Vec<String> {
    vec![
        "--add-headers".to_string(),
        format!("Referer:{BILIBILI_REFERER}"),
        "--add-headers".to_string(),
        "Origin:https://www.bilibili.com".to_string(),
        "--add-headers".to_string(),
        format!("User-Agent:{BILIBILI_USER_AGENT}"),
    ]
}

fn bilibili_cookie_browsers() -> Vec<&'static str> {
    #[cfg(target_os = "macos")]
    let browsers = vec!["safari", "chrome", "edge", "firefox"];
    #[cfg(target_os = "windows")]
    let browsers = vec!["edge", "chrome", "chromium", "brave", "firefox"];
    #[cfg(all(unix, not(target_os = "macos")))]
    let browsers = vec!["chrome", "chromium", "firefox"];

    browsers
        .into_iter()
        .filter(|browser| browser_cookie_store_available(browser))
        .collect()
}

#[cfg(target_os = "windows")]
fn browser_cookie_store_available(browser: &str) -> bool {
    let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let roaming_app_data = env::var_os("APPDATA").map(PathBuf::from);

    match browser {
        "edge" => local_app_data
            .as_ref()
            .is_some_and(|path| path.join("Microsoft/Edge/User Data").exists()),
        "chrome" => local_app_data
            .as_ref()
            .is_some_and(|path| path.join("Google/Chrome/User Data").exists()),
        "chromium" => local_app_data
            .as_ref()
            .is_some_and(|path| path.join("Chromium/User Data").exists()),
        "brave" => local_app_data.as_ref().is_some_and(|path| {
            path.join("BraveSoftware/Brave-Browser/User Data").exists()
        }),
        "firefox" => {
            roaming_app_data
                .as_ref()
                .is_some_and(|path| path.join("Mozilla/Firefox/Profiles").exists())
                || local_app_data.as_ref().is_some_and(|path| {
                    path.join(
                        "Packages/Mozilla.Firefox_n80bbvh6b1yt2/LocalCache/Roaming/Mozilla/Firefox/Profiles",
                    )
                    .exists()
                })
        }
        _ => true,
    }
}

#[cfg(not(target_os = "windows"))]
fn browser_cookie_store_available(_browser: &str) -> bool {
    true
}

fn is_bilibili_url(url: &str) -> bool {
    let lowered = url.to_ascii_lowercase();
    lowered.contains("bilibili.com") || lowered.contains("b23.tv")
}

fn select_preferred_download_error(previous: String, next: String) -> String {
    let previous_score = score_download_error(&previous);
    let next_score = score_download_error(&next);
    if next_score > previous_score {
        next
    } else {
        previous
    }
}

fn score_download_error(message: &str) -> u8 {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("could not find")
        && (lowered.contains("cookies database") || lowered.contains("cookies file"))
    {
        return 0;
    }
    if lowered.contains("http error")
        || lowered.contains("403")
        || lowered.contains("unable to download")
        || lowered.contains("download json metadata")
    {
        return 3;
    }
    if lowered.contains("login") || lowered.contains("cookie") {
        return 2;
    }
    if lowered.contains("browser") || lowered.contains("firefox") || lowered.contains("chrome") {
        return 1;
    }
    0
}

fn read_yt_dlp_pipe<R: Read + Send + 'static>(
    reader: R,
    is_stdout: bool,
    sender: mpsc::Sender<YtDlpPipeLine>,
) {
    let reader = BufReader::new(reader);
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        if sender.send(YtDlpPipeLine { is_stdout, line }).is_err() {
            break;
        }
    }
}

fn parse_yt_dlp_percent(line: &str) -> Option<f32> {
    if !line.contains("[download]") || !line.contains('%') {
        return None;
    }

    let percent_index = line.find('%')?;
    let prefix = &line[..percent_index];
    let start = prefix
        .rfind(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .map(|index| index + 1)
        .unwrap_or(0);
    let number = prefix[start..].trim();
    if number.is_empty() {
        return None;
    }

    number
        .parse::<f32>()
        .ok()
        .map(|value| value.clamp(0.0, 100.0))
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

fn copy_file_with_progress(
    app: &AppHandle,
    source_path: &Path,
    target_path: &Path,
) -> Result<(), String> {
    let total_bytes = fs::metadata(source_path)
        .map_err(|error| format!("读取源文件信息失败: {error}"))?
        .len();
    let mut source = File::open(source_path).map_err(|error| format!("打开源文件失败: {error}"))?;
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
        Err(format!("视频转音频失败: {}", detail.trim()))
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
