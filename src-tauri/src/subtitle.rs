use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::media::{self, MediaItem};
use crate::sidecar;

const TRANSLATION_TARGET_LANGUAGE: &str = "zh";
const TRANSLATION_SCRIPT_NAME: &str = "translate.py";
const UV_BIN_ENV: &str = "UV_BIN";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleCue {
    pub id: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub secondary_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDocument {
    pub media_id: String,
    pub title: String,
    pub subtitle_path: String,
    pub cues: Vec<SubtitleCue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSubtitleDocument {
    version: u8,
    cues: Vec<SubtitleCue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationResponse {
    source_language: String,
    translations: Vec<String>,
}

pub fn get_subtitle_document(app: &AppHandle, media_id: &str) -> Result<SubtitleDocument, String> {
    let media_item = find_media_item(app, media_id)?;
    let subtitle_path = media_item
        .subtitle_path
        .clone()
        .ok_or_else(|| "该素材还没有可编辑字幕".to_string())?;
    let cues = load_cues_from_path(&subtitle_path)?;

    Ok(SubtitleDocument {
        media_id: media_item.id,
        title: media_item.title,
        subtitle_path,
        cues,
    })
}

pub fn save_subtitle_document(
    app: &AppHandle,
    media_id: &str,
    cues: Vec<SubtitleCue>,
) -> Result<SubtitleDocument, String> {
    let normalized_cues = normalize_cues(cues);
    if normalized_cues.is_empty() {
        return Err("字幕内容为空，无法保存".to_string());
    }

    let subtitle_path = build_local_subtitle_path(app, media_id)?;
    let content = serde_json::to_string_pretty(&StoredSubtitleDocument {
        version: 1,
        cues: normalized_cues.clone(),
    })
    .map_err(|error| format!("序列化字幕数据失败: {error}"))?;

    fs::write(&subtitle_path, content).map_err(|error| format!("写入字幕文件失败: {error}"))?;
    let updated_item =
        media::update_media_subtitle(app, media_id, &subtitle_path.display().to_string())?;

    Ok(SubtitleDocument {
        media_id: updated_item.id,
        title: updated_item.title,
        subtitle_path: subtitle_path.display().to_string(),
        cues: normalized_cues,
    })
}

pub fn translate_media_subtitle(
    app: &AppHandle,
    media_id: &str,
) -> Result<SubtitleDocument, String> {
    let document = get_subtitle_document(app, media_id)?;
    if document.cues.is_empty() {
        return Err("没有可翻译的字幕内容".to_string());
    }

    let translated_cues = translate_cues(app, &document.cues)?;
    save_subtitle_document(app, media_id, translated_cues)
}

fn find_media_item(app: &AppHandle, media_id: &str) -> Result<MediaItem, String> {
    media::get_library_state(app)?
        .media_items
        .into_iter()
        .find(|item| item.id == media_id)
        .ok_or_else(|| "未找到对应素材".to_string())
}

fn build_local_subtitle_path(app: &AppHandle, media_id: &str) -> Result<PathBuf, String> {
    let subtitle_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("subtitles");
    fs::create_dir_all(&subtitle_dir).map_err(|error| format!("创建字幕目录失败: {error}"))?;
    Ok(subtitle_dir.join(format!("{media_id}.json")))
}

fn load_cues_from_path(path: &str) -> Result<Vec<SubtitleCue>, String> {
    let subtitle_path = PathBuf::from(path);
    if !subtitle_path.exists() {
        return Err("字幕文件不存在".to_string());
    }

    let content =
        fs::read_to_string(&subtitle_path).map_err(|error| format!("读取字幕文件失败: {error}"))?;
    match subtitle_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => parse_json_subtitle(&content),
        _ => parse_text_subtitle(&content),
    }
}

fn parse_json_subtitle(content: &str) -> Result<Vec<SubtitleCue>, String> {
    if let Ok(stored) = serde_json::from_str::<StoredSubtitleDocument>(content) {
        return Ok(normalize_cues(stored.cues));
    }

    if let Ok(cues) = serde_json::from_str::<Vec<SubtitleCue>>(content) {
        return Ok(normalize_cues(cues));
    }

    Err("解析 JSON 字幕失败".to_string())
}

fn parse_text_subtitle(content: &str) -> Result<Vec<SubtitleCue>, String> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace('\r', "")
        .trim()
        .to_string();

    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let without_header = if normalized.starts_with("WEBVTT") {
        normalized
            .split_once('\n')
            .map(|(_, rest)| rest.trim_start_matches('\n').to_string())
            .unwrap_or_default()
    } else {
        normalized
    };

    let mut cues = Vec::new();

    for block in without_header.split("\n\n") {
        if let Some(cue) = parse_text_cue(block, (cues.len() + 1) as u32)? {
            cues.push(cue);
        }
    }

    Ok(cues)
}

fn parse_text_cue(block: &str, fallback_id: u32) -> Result<Option<SubtitleCue>, String> {
    let lines = block
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return Ok(None);
    }

    let marker = lines[0].trim();
    if marker.starts_with("NOTE") || marker.starts_with("STYLE") || marker.starts_with("REGION") {
        return Ok(None);
    }

    let Some(timeline_index) = lines.iter().position(|line| line.contains("-->")) else {
        return Ok(None);
    };

    let timeline = lines[timeline_index];
    let Some((raw_start, raw_end_with_settings)) = timeline.split_once("-->") else {
        return Ok(None);
    };
    let raw_end = raw_end_with_settings
        .split_whitespace()
        .next()
        .ok_or_else(|| "字幕时间轴格式错误".to_string())?;
    let start_ms = parse_timestamp(raw_start)?;
    let end_ms = parse_timestamp(raw_end)?;
    let content_lines = lines
        .iter()
        .skip(timeline_index + 1)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if content_lines.is_empty() {
        return Ok(None);
    }

    let (text, secondary_text) = split_subtitle_lines(&content_lines);

    Ok(Some(SubtitleCue {
        id: fallback_id,
        start_ms,
        end_ms,
        text,
        secondary_text,
    }))
}

fn split_subtitle_lines(lines: &[&str]) -> (String, Option<String>) {
    if lines.len() == 1 {
        return (lines[0].to_string(), None);
    }

    let last_line = lines[lines.len() - 1];
    if contains_chinese(last_line) {
        let primary = lines[..lines.len() - 1].join("\n");
        return (primary, Some(last_line.to_string()));
    }

    (lines.join("\n"), None)
}

fn parse_timestamp(token: &str) -> Result<u64, String> {
    let normalized = token.trim().replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();

    if !(2..=3).contains(&parts.len()) {
        return Err(format!("非法时间戳: {token}"));
    }

    let (hours, minutes, seconds_with_millis) = if parts.len() == 3 {
        (parts[0], parts[1], parts[2])
    } else {
        ("0", parts[0], parts[1])
    };

    let seconds_parts = seconds_with_millis.split('.').collect::<Vec<_>>();
    let seconds = seconds_parts[0]
        .parse::<u64>()
        .map_err(|_| format!("非法时间戳: {token}"))?;
    let millis_token = format!("{}000", seconds_parts.get(1).copied().unwrap_or("0"));
    let millis = millis_token[..3]
        .parse::<u64>()
        .map_err(|_| format!("非法时间戳: {token}"))?;

    let hours = hours
        .parse::<u64>()
        .map_err(|_| format!("非法时间戳: {token}"))?;
    let minutes = minutes
        .parse::<u64>()
        .map_err(|_| format!("非法时间戳: {token}"))?;

    Ok((((hours * 60) + minutes) * 60 + seconds) * 1000 + millis)
}

fn normalize_cues(cues: Vec<SubtitleCue>) -> Vec<SubtitleCue> {
    let mut normalized = cues
        .into_iter()
        .filter(|cue| !cue.text.trim().is_empty())
        .collect::<Vec<_>>();

    normalized.sort_by_key(|cue| cue.start_ms);
    for (index, cue) in normalized.iter_mut().enumerate() {
        cue.id = (index + 1) as u32;
        cue.text = cue.text.trim().to_string();
        cue.secondary_text = cue
            .secondary_text
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
    }

    normalized
}

fn translate_cues(app: &AppHandle, cues: &[SubtitleCue]) -> Result<Vec<SubtitleCue>, String> {
    let source_lines = cues
        .iter()
        .map(|cue| cue.text.trim().to_string())
        .collect::<Vec<_>>();

    let translation = request_offline_translation(app, &source_lines)?;
    if translation.translations.len() != cues.len() {
        return Err("中文字幕数量与原文字幕数量不一致".to_string());
    }

    if translation.source_language == TRANSLATION_TARGET_LANGUAGE {
        return Ok(cues
            .iter()
            .map(|cue| SubtitleCue {
                id: cue.id,
                start_ms: cue.start_ms,
                end_ms: cue.end_ms,
                text: cue.text.clone(),
                secondary_text: Some(cue.text.clone()),
            })
            .collect());
    }

    Ok(cues
        .iter()
        .zip(translation.translations)
        .map(|(cue, translated)| SubtitleCue {
            id: cue.id,
            start_ms: cue.start_ms,
            end_ms: cue.end_ms,
            text: cue.text.clone(),
            secondary_text: Some(translated),
        })
        .collect())
}

fn request_offline_translation(
    app: &AppHandle,
    lines: &[String],
) -> Result<TranslationResponse, String> {
    let translator_project = resolve_translator_project_dir(app)?;
    let translator_script = translator_project.join(TRANSLATION_SCRIPT_NAME);
    let argos_packages_dir = resolve_argos_packages_dir(app)?;
    let bundled_model_dirs = resolve_argos_model_dirs(app);
    let model_dir_env = std::env::join_paths(&bundled_model_dirs)
        .map_err(|error| format!("拼接离线翻译模型目录失败: {error}"))?;

    let request_payload = json!({
        "targetLanguage": TRANSLATION_TARGET_LANGUAGE,
        "lines": lines,
    })
    .to_string();

    let uv_bin = env::var(UV_BIN_ENV).unwrap_or_else(|_| "uv".to_string());
    let uv_target = sidecar::CommandTarget::Program(uv_bin);
    // 使用 build_nice_command 降低翻译进程 CPU 优先级，避免 UI 卡顿
    // macOS: taskpolicy -b，Linux: nice -n 19
    let mut child = sidecar::build_nice_command(&uv_target)
        .args([
            "run",
            "--project",
            &translator_project.display().to_string(),
            &translator_script.display().to_string(),
        ])
        .env("ARGOS_PACKAGES_DIR", &argos_packages_dir)
        .env("ARGOS_DEVICE_TYPE", "cpu")
        .env("OFFLINE_TRANSLATOR_MODEL_DIRS", model_dir_env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("启动离线翻译进程失败，请确认已安装 uv: {error}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(request_payload.as_bytes())
            .map_err(|error| format!("写入离线翻译请求失败: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待离线翻译进程失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            "离线翻译进程执行失败".to_string()
        } else {
            format!("离线翻译进程执行失败: {detail}")
        });
    }

    serde_json::from_slice::<TranslationResponse>(&output.stdout)
        .map_err(|error| format!("解析离线翻译结果失败: {error}"))
}

fn resolve_translator_project_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(resource_dir) = app
        .path()
        .resolve("scripts/offline_translator", BaseDirectory::Resource)
    {
        if resource_dir.exists() {
            return Ok(resource_dir);
        }
    }

    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    let candidates = [
        current_dir.join("scripts/offline_translator"),
        current_dir
            .parent()
            .map(|dir| dir.join("scripts/offline_translator"))
            .unwrap_or_else(|| current_dir.join("scripts/offline_translator")),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| "未找到离线翻译脚本目录".to_string())
}

fn resolve_argos_packages_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let packages_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("argos/packages");
    fs::create_dir_all(&packages_dir)
        .map_err(|error| format!("创建离线翻译模型目录失败: {error}"))?;
    Ok(packages_dir)
}

fn resolve_argos_model_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut model_dirs = Vec::new();

    if let Ok(resource_dir) = app
        .path()
        .resolve("models/argos", BaseDirectory::Resource)
    {
        if resource_dir.exists() {
            model_dirs.push(resource_dir);
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        let direct = current_dir.join("models/argos");
        if direct.exists() {
            model_dirs.push(direct);
        }

        let fallback = current_dir.join("models");
        if fallback.exists() {
            model_dirs.push(fallback);
        }

        if let Some(parent) = current_dir.parent() {
            let parent_dir = parent.join("models/argos");
            if parent_dir.exists() {
                model_dirs.push(parent_dir);
            }
        }
    }

    model_dirs
}

fn contains_chinese(text: &str) -> bool {
    text.chars().any(|char| ('\u{4e00}'..='\u{9fff}').contains(&char))
}

#[allow(dead_code)]
fn _subtitle_path_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
