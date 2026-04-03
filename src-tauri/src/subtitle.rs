use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::media::{self, MediaItem};
use crate::sidecar;
use crate::state::ExternalProcessState;

const TRANSLATION_TARGET_LANGUAGE: &str = "zh";
const TRANSLATION_MODEL_DIR_ENV: &str = "MUYU_TRANSLATION_MODEL_DIR";
const TRANSLATION_MODEL_LAYOUT_NAME: &str = "m2m100_418m";
const MAX_SEGMENT_DURATION_MS: u64 = 2_600;
const MIN_SEGMENT_DURATION_MS: u64 = 700;
const MAX_LATIN_SEGMENT_CHARS: usize = 26;
const MAX_CJK_SEGMENT_CHARS: usize = 16;
const MIN_SPLIT_VISIBLE_CHARS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleAtom {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleCue {
    pub id: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub secondary_text: Option<String>,
    #[serde(default)]
    pub atoms: Vec<SubtitleAtom>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WhisperJsonDocument {
    transcription: Vec<WhisperSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WhisperSegment {
    offsets: WhisperOffsets,
    text: String,
    #[serde(default)]
    tokens: Vec<WhisperToken>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WhisperOffsets {
    from: u64,
    to: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WhisperToken {
    text: String,
    t0: Option<i64>,
    t1: Option<i64>,
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
    title: &str,
    cues: Vec<SubtitleCue>,
) -> Result<SubtitleDocument, String> {
    let normalized_cues = normalize_cues(cues);
    if normalized_cues.is_empty() {
        return Err("字幕内容为空，无法保存".to_string());
    }

    let subtitle_path = build_local_subtitle_path(app, media_id)?;
    let content = serde_json::to_string_pretty(&StoredSubtitleDocument {
        version: 2,
        cues: normalized_cues.clone(),
    })
    .map_err(|error| format!("序列化字幕数据失败: {error}"))?;

    fs::write(&subtitle_path, content).map_err(|error| format!("写入字幕文件失败: {error}"))?;
    media::update_media_subtitle(app, media_id, &subtitle_path.display().to_string())?;
    let updated_item = media::update_media_title(app, media_id, title)?;

    Ok(SubtitleDocument {
        media_id: updated_item.id,
        title: updated_item.title,
        subtitle_path: subtitle_path.display().to_string(),
        cues: normalized_cues,
    })
}

pub fn translate_media_subtitle(
    app: &AppHandle,
    active_translation_job: Arc<Mutex<Option<ExternalProcessState>>>,
    media_id: &str,
    source_language: Option<&str>,
) -> Result<SubtitleDocument, String> {
    let document = get_subtitle_document(app, media_id)?;
    if document.cues.is_empty() {
        return Err("没有可翻译的字幕内容".to_string());
    }

    let translated_cues =
        translate_cues(app, active_translation_job, &document.cues, source_language)?;
    save_subtitle_document(app, media_id, &document.title, translated_cues)
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

pub fn write_generated_subtitle_document(
    path: &Path,
    cues: Vec<SubtitleCue>,
) -> Result<Vec<SubtitleCue>, String> {
    let normalized_cues = normalize_cues(cues);
    if normalized_cues.is_empty() {
        return Err("字幕内容为空，无法写入".to_string());
    }

    let content = serde_json::to_string_pretty(&StoredSubtitleDocument {
        version: 2,
        cues: normalized_cues.clone(),
    })
    .map_err(|error| format!("序列化字幕数据失败: {error}"))?;

    fs::write(path, content).map_err(|error| format!("写入字幕文件失败: {error}"))?;
    Ok(normalized_cues)
}

pub fn build_cues_from_asr_outputs(
    subtitle_path: &Path,
    whisper_json_path: Option<&Path>,
) -> Result<Vec<SubtitleCue>, String> {
    let raw =
        fs::read(subtitle_path).map_err(|error| format!("读取字幕文件失败: {error}"))?;
    let content = String::from_utf8_lossy(&raw);
    let base_cues = parse_text_subtitle(&content)?;

    let Some(json_path) = whisper_json_path.filter(|path| path.exists()) else {
        return Ok(normalize_cues(base_cues));
    };

    let whisper_json = fs::read_to_string(json_path)
        .map_err(|error| format!("读取词级时间戳文件失败: {error}"))?;
    let atoms = parse_whisper_timed_atoms(&whisper_json)?;
    Ok(attach_atoms_to_cues(base_cues, atoms))
}

fn load_cues_from_path(path: &str) -> Result<Vec<SubtitleCue>, String> {
    let subtitle_path = PathBuf::from(path);
    if !subtitle_path.exists() {
        return Err("字幕文件不存在".to_string());
    }

    let extension = subtitle_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "json" => {
            let content = fs::read_to_string(&subtitle_path)
                .map_err(|error| format!("读取字幕文件失败: {error}"))?;
            parse_json_subtitle(&content)
        }
        _ => {
            let raw =
                fs::read(&subtitle_path).map_err(|error| format!("读取字幕文件失败: {error}"))?;
            let content = String::from_utf8_lossy(&raw);
            parse_text_subtitle(&content)
        }
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
        .replace('\u{85}', "\n")
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
        atoms: Vec::new(),
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
    let mut split = Vec::new();
    for cue in normalized.into_iter() {
        split.extend(split_long_cue(cue));
    }

    for (index, cue) in split.iter_mut().enumerate() {
        cue.id = (index + 1) as u32;
        cue.text = normalize_subtitle_text(&cue.text);
        cue.secondary_text = cue
            .secondary_text
            .as_ref()
            .map(|text| normalize_subtitle_text(text))
            .filter(|text| !text.is_empty());
        cue.atoms = normalize_atoms(&cue.atoms, cue.start_ms, cue.end_ms);
        if cue.atoms.is_empty() || !atoms_match_text(&cue.atoms, &cue.text) {
            cue.atoms = build_fallback_atoms(&cue.text, cue.start_ms, cue.end_ms);
        }
    }

    split
}

fn split_long_cue(cue: SubtitleCue) -> Vec<SubtitleCue> {
    if !cue.atoms.is_empty() {
        return vec![cue];
    }

    let text = normalize_subtitle_text(&cue.text);
    if text.is_empty() {
        return Vec::new();
    }

    let secondary_text = cue
        .secondary_text
        .as_ref()
        .map(|value| normalize_subtitle_text(value))
        .filter(|value| !value.is_empty());

    let duration = cue.end_ms.saturating_sub(cue.start_ms);
    let segment_count = estimate_segment_count(&text, duration);
    if segment_count <= 1 {
        return vec![SubtitleCue {
            text,
            secondary_text,
            atoms: Vec::new(),
            ..cue
        }];
    }

    let primary_segments = split_text_into_segments(&text, segment_count);
    if primary_segments.len() <= 1 {
        return vec![SubtitleCue {
            text,
            secondary_text,
            atoms: Vec::new(),
            ..cue
        }];
    }

    let secondary_segments = secondary_text
        .as_ref()
        .map(|value| split_text_into_segments(value, primary_segments.len()));
    let weights = primary_segments
        .iter()
        .map(|segment| count_visible_chars(segment).max(1))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<usize>().max(1);
    let mut remaining_duration = duration;
    let mut remaining_weight = total_weight;
    let mut segment_start = cue.start_ms;
    let segment_len = primary_segments.len();

    primary_segments
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let segment_end = if index + 1 == segment_len {
                cue.end_ms
            } else {
                let remaining_segments = segment_len - index;
                let ideal_duration =
                    ((remaining_duration as f64) * (weights[index] as f64) / (remaining_weight as f64))
                        .round() as u64;
                let min_duration = MIN_SEGMENT_DURATION_MS;
                let max_duration = remaining_duration
                    .saturating_sub(min_duration.saturating_mul((remaining_segments - 1) as u64));
                let segment_duration = ideal_duration.clamp(min_duration, max_duration.max(min_duration));
                segment_start.saturating_add(segment_duration)
            };

            let cue = SubtitleCue {
                id: cue.id,
                start_ms: segment_start,
                end_ms: segment_end,
                text,
                secondary_text: secondary_segments
                    .as_ref()
                    .and_then(|segments| segments.get(index).cloned())
                    .filter(|value| !value.is_empty()),
                atoms: Vec::new(),
            };

            remaining_duration = remaining_duration.saturating_sub(segment_end.saturating_sub(segment_start));
            remaining_weight = remaining_weight.saturating_sub(weights[index]);
            segment_start = segment_end;
            cue
        })
        .collect()
}

fn normalize_atoms(atoms: &[SubtitleAtom], cue_start_ms: u64, cue_end_ms: u64) -> Vec<SubtitleAtom> {
    let mut normalized = atoms
        .iter()
        .filter_map(|atom| {
            let text = normalize_subtitle_text(&atom.text);
            if text.is_empty() {
                return None;
            }

            let start_ms = atom.start_ms.max(cue_start_ms).min(cue_end_ms);
            let end_ms = atom.end_ms.max(start_ms).min(cue_end_ms);

            Some(SubtitleAtom {
                text,
                start_ms,
                end_ms,
            })
        })
        .collect::<Vec<_>>();

    normalized.sort_by_key(|atom| atom.start_ms);
    normalized
}

fn atoms_match_text(atoms: &[SubtitleAtom], text: &str) -> bool {
    let atom_units = atoms
        .iter()
        .map(|atom| normalize_subtitle_text(&atom.text))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    atom_units == text_units(text)
}

fn build_fallback_atoms(text: &str, start_ms: u64, end_ms: u64) -> Vec<SubtitleAtom> {
    let units = text_units(text);
    if units.is_empty() {
        return Vec::new();
    }

    if units.len() == 1 {
        return vec![SubtitleAtom {
            text: normalize_subtitle_text(text),
            start_ms,
            end_ms,
        }];
    }

    let weights = units
        .iter()
        .map(|unit| count_visible_chars(unit).max(1))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<usize>().max(1);
    let duration = end_ms.saturating_sub(start_ms);
    let mut remaining_duration = duration;
    let mut remaining_weight = total_weight;
    let mut cursor = start_ms;
    let last_index = units.len().saturating_sub(1);

    units
        .into_iter()
        .enumerate()
        .map(|(index, unit)| {
            let atom_end = if index == last_index {
                end_ms
            } else {
                let remaining_units = last_index - index;
                let ideal_duration =
                    ((remaining_duration as f64) * (weights[index] as f64) / (remaining_weight as f64))
                        .round() as u64;
                let min_duration = if remaining_duration > 0 {
                    (remaining_duration / ((remaining_units + 1) as u64)).max(1)
                } else {
                    0
                };
                let max_duration = remaining_duration
                    .saturating_sub(min_duration.saturating_mul(remaining_units as u64));
                cursor.saturating_add(ideal_duration.clamp(min_duration, max_duration.max(min_duration)))
            };

            let atom = SubtitleAtom {
                text: unit,
                start_ms: cursor,
                end_ms: atom_end,
            };

            remaining_duration = remaining_duration.saturating_sub(atom_end.saturating_sub(cursor));
            remaining_weight = remaining_weight.saturating_sub(weights[index]);
            cursor = atom_end;
            atom
        })
        .collect()
}

fn estimate_segment_count(text: &str, duration_ms: u64) -> usize {
    if duration_ms < MIN_SEGMENT_DURATION_MS * 2 {
        return 1;
    }

    let visible_chars = count_visible_chars(text);
    if visible_chars < MIN_SPLIT_VISIBLE_CHARS {
        return 1;
    }

    let max_chars = if contains_cjk(text) {
        MAX_CJK_SEGMENT_CHARS
    } else {
        MAX_LATIN_SEGMENT_CHARS
    };
    let duration_segments = duration_ms.div_ceil(MAX_SEGMENT_DURATION_MS) as usize;
    let length_segments = visible_chars.div_ceil(max_chars);
    let max_segments = (duration_ms / MIN_SEGMENT_DURATION_MS).max(1) as usize;

    duration_segments.max(length_segments).clamp(1, max_segments)
}

fn split_text_into_segments(text: &str, requested_segments: usize) -> Vec<String> {
    let units = text_units(text);
    if units.len() <= 1 || requested_segments <= 1 {
        return vec![text.to_string()];
    }

    let segment_count = requested_segments.min(units.len());
    let weights = units
        .iter()
        .map(|unit| count_visible_chars(unit).max(1))
        .collect::<Vec<_>>();
    let mut remaining_weight = weights.iter().sum::<usize>();
    let mut cursor = 0usize;
    let mut segments = Vec::with_capacity(segment_count);

    for segment_index in 0..segment_count {
        let remaining_segments = segment_count - segment_index;
        let remaining_units = units.len() - cursor;

        if remaining_segments == 1 {
            segments.push(join_text_units(&units[cursor..]));
            break;
        }

        let target_weight = (remaining_weight as f64) / (remaining_segments as f64);
        let mut current_weight = 0usize;
        let mut end = cursor;

        while end < units.len() {
            let units_left_after_pick = units.len() - (end + 1);
            if units_left_after_pick < remaining_segments - 1 {
                break;
            }

            let next_weight = current_weight + weights[end];
            let should_stop = end > cursor
                && ((next_weight as f64) - target_weight).abs()
                    > ((current_weight as f64) - target_weight).abs();
            if should_stop {
                break;
            }

            current_weight = next_weight;
            end += 1;

            if current_weight >= target_weight as usize && remaining_units > remaining_segments {
                break;
            }
        }

        if end == cursor {
            end += 1;
            current_weight = weights[cursor];
        }

        segments.push(join_text_units(&units[cursor..end]));
        cursor = end;
        remaining_weight = remaining_weight.saturating_sub(current_weight);
    }

    segments
        .into_iter()
        .map(|segment| normalize_subtitle_text(&segment))
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn text_units(text: &str) -> Vec<String> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.len() > 1 {
        return words.into_iter().map(str::to_string).collect();
    }

    let mut units: Vec<String> = Vec::new();
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }

        if is_split_punctuation(ch) && !units.is_empty() {
            units
                .last_mut()
                .expect("units is not empty")
                .push(ch);
        } else {
            units.push(ch.to_string());
        }
    }

    units
}

fn join_text_units(units: &[String]) -> String {
    if units.iter().any(|unit| unit.contains(' ')) {
        return units.join(" ");
    }

    if units.iter().all(|unit| unit.chars().count() == 1 || unit.chars().last().map(is_split_punctuation).unwrap_or(false)) {
        return units.join("");
    }

    units.join(" ")
}

fn normalize_subtitle_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn count_visible_chars(text: &str) -> usize {
    text.chars().filter(|ch| !ch.is_whitespace()).count()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|char| {
        ('\u{4e00}'..='\u{9fff}').contains(&char)
            || ('\u{3040}'..='\u{30ff}').contains(&char)
            || ('\u{ac00}'..='\u{d7af}').contains(&char)
    })
}

fn is_split_punctuation(ch: char) -> bool {
    matches!(
        ch,
        ',' | '.' | '!' | '?' | ';' | ':' | '，' | '。' | '！' | '？' | '；' | '：' | '、'
    )
}

fn parse_whisper_timed_atoms(content: &str) -> Result<Vec<SubtitleAtom>, String> {
    let document = serde_json::from_str::<WhisperJsonDocument>(content)
        .map_err(|error| format!("解析 whisper 词级时间戳失败: {error}"))?;

    let mut atoms = Vec::new();

    for segment in document.transcription {
        let segment_start_ms = segment.offsets.from.saturating_mul(10);
        let segment_end_ms = segment.offsets.to.saturating_mul(10);
        let segment_atoms = build_atoms_from_whisper_tokens(
            &segment.tokens,
            segment_start_ms,
            segment_end_ms,
            &segment.text,
        );
        atoms.extend(segment_atoms);
    }

    atoms.sort_by_key(|atom| atom.start_ms);
    Ok(atoms)
}

fn build_atoms_from_whisper_tokens(
    tokens: &[WhisperToken],
    segment_start_ms: u64,
    segment_end_ms: u64,
    fallback_text: &str,
) -> Vec<SubtitleAtom> {
    let mut atoms: Vec<SubtitleAtom> = Vec::new();

    for token in tokens {
        let Some(token_start_ms) = token.t0.and_then(whisper_time_to_ms) else {
            continue;
        };
        let Some(token_end_ms) = token.t1.and_then(whisper_time_to_ms) else {
            continue;
        };

        let raw_text = token.text.replace('\n', " ");
        let text = normalize_subtitle_text(&raw_text);
        if text.is_empty() {
            continue;
        }

        let starts_with_whitespace = raw_text.chars().next().map(|char| char.is_whitespace()).unwrap_or(false);
        let is_punctuation = text.chars().all(is_split_punctuation);
        let looks_like_cjk_unit = contains_cjk(&text) && text.chars().count() <= 2;

        if is_punctuation {
            if let Some(previous) = atoms.last_mut() {
                previous.text.push_str(&text);
                previous.end_ms = token_end_ms.max(previous.end_ms);
            } else {
                atoms.push(SubtitleAtom {
                    text,
                    start_ms: token_start_ms,
                    end_ms: token_end_ms.max(token_start_ms),
                });
            }
            continue;
        }

        if !starts_with_whitespace && !looks_like_cjk_unit {
            if let Some(previous) = atoms.last_mut() {
                previous.text.push_str(&text);
                previous.end_ms = token_end_ms.max(previous.end_ms);
                continue;
            }
        }

        atoms.push(SubtitleAtom {
            text,
            start_ms: token_start_ms,
            end_ms: token_end_ms.max(token_start_ms),
        });
    }

    let normalized = normalize_atoms(&atoms, segment_start_ms, segment_end_ms);
    if normalized.is_empty() {
        return build_fallback_atoms(fallback_text, segment_start_ms, segment_end_ms);
    }

    normalized
}

fn whisper_time_to_ms(value: i64) -> Option<u64> {
    if value < 0 {
        None
    } else {
        Some((value as u64).saturating_mul(10))
    }
}

fn attach_atoms_to_cues(cues: Vec<SubtitleCue>, atoms: Vec<SubtitleAtom>) -> Vec<SubtitleCue> {
    let mut normalized = normalize_cues(cues);
    if atoms.is_empty() {
        return normalized;
    }

    let mut cursor = 0usize;
    for cue in &mut normalized {
        while cursor < atoms.len() && atoms[cursor].end_ms <= cue.start_ms {
            cursor += 1;
        }

        let mut overlapping = Vec::new();
        let mut index = cursor;
        while index < atoms.len() && atoms[index].start_ms < cue.end_ms {
            if atoms[index].end_ms > cue.start_ms {
                overlapping.push(atoms[index].clone());
            }
            index += 1;
        }

        let fitted = fit_atoms_to_text(&cue.text, cue.start_ms, cue.end_ms, &overlapping);
        if !fitted.is_empty() {
            cue.atoms = fitted;
        }
    }

    normalized
}

fn fit_atoms_to_text(
    text: &str,
    cue_start_ms: u64,
    cue_end_ms: u64,
    source_atoms: &[SubtitleAtom],
) -> Vec<SubtitleAtom> {
    let normalized_source = normalize_atoms(source_atoms, cue_start_ms, cue_end_ms);
    if normalized_source.is_empty() {
        return build_fallback_atoms(text, cue_start_ms, cue_end_ms);
    }

    if atoms_match_text(&normalized_source, text) {
        return normalized_source;
    }

    let units = text_units(text);
    if units.is_empty() {
        return Vec::new();
    }

    if units.len() == normalized_source.len() {
        return normalized_source
            .into_iter()
            .zip(units)
            .map(|(atom, unit)| SubtitleAtom {
                text: unit,
                start_ms: atom.start_ms,
                end_ms: atom.end_ms,
            })
            .collect();
    }

    build_fallback_atoms(text, cue_start_ms, cue_end_ms)
}

fn translate_cues(
    app: &AppHandle,
    active_translation_job: Arc<Mutex<Option<ExternalProcessState>>>,
    cues: &[SubtitleCue],
    source_language: Option<&str>,
) -> Result<Vec<SubtitleCue>, String> {
    let source_lines = cues
        .iter()
        .map(|cue| cue.text.trim().to_string())
        .collect::<Vec<_>>();

    let translation =
        request_offline_translation(app, active_translation_job, &source_lines, source_language)?;
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
                secondary_text: None,
                atoms: cue.atoms.clone(),
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
            atoms: cue.atoms.clone(),
        })
        .collect())
}

fn request_offline_translation(
    app: &AppHandle,
    active_translation_job: Arc<Mutex<Option<ExternalProcessState>>>,
    lines: &[String],
    source_language: Option<&str>,
) -> Result<TranslationResponse, String> {
    let translator = sidecar::locate_executable(app, "TRANSLATOR_CLI_BIN", &["translator-cli"])
        .map_err(|error| format!("未找到 translator-cli sidecar: {error}"))?;
    let translation_model_dir = resolve_translation_model_dir(app)?;

    let request_payload = json!({
        "targetLanguage": TRANSLATION_TARGET_LANGUAGE,
        "sourceLanguage": source_language,
        "lines": lines,
    })
    .to_string();

    let translator_path = translator.display();
    let mut child = sidecar::spawn_command_with_priority(&translator, |command| {
        command
            .env(TRANSLATION_MODEL_DIR_ENV, &translation_model_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    })
        .map_err(|error| format!("启动 translator-cli 失败（{translator_path}）: {error}"))?;
    let pid = child.id();
    if let Ok(mut guard) = active_translation_job.lock() {
        *guard = Some(ExternalProcessState::new("字幕翻译", pid));
    }

    let result = (|| -> Result<TranslationResponse, String> {
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
    })();

    if let Ok(mut guard) = active_translation_job.lock() {
        if guard.as_ref().map(|task| task.pid == pid).unwrap_or(false) {
            *guard = None;
        }
    }

    result
}

fn resolve_translation_model_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(value) = env::var(TRANSLATION_MODEL_DIR_ENV) {
        let path = PathBuf::from(value);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(app_data_dir) = app.path().app_data_dir() {
        for candidate in [
            app_data_dir
                .join("models/translation")
                .join(TRANSLATION_MODEL_LAYOUT_NAME),
            app_data_dir.join("models/translation"),
        ] {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    for resource_path in [
        format!("models/translation/{TRANSLATION_MODEL_LAYOUT_NAME}"),
        "models/translation".to_string(),
        format!("_up_/models/translation/{TRANSLATION_MODEL_LAYOUT_NAME}"),
        "_up_/models/translation".to_string(),
    ] {
        if let Ok(resource_dir) = app.path().resolve(&resource_path, BaseDirectory::Resource) {
            if resource_dir.exists() {
                return Ok(resource_dir);
            }
        }
    }

    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    let mut candidates = vec![
        current_dir
            .join("models/translation")
            .join(TRANSLATION_MODEL_LAYOUT_NAME),
        current_dir.join("models/translation"),
    ];

    if let Some(parent) = current_dir.parent() {
        candidates.push(
            parent
                .join("models/translation")
                .join(TRANSLATION_MODEL_LAYOUT_NAME),
        );
        candidates.push(parent.join("models/translation"));
    }

    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            "未找到原生离线翻译模型目录，请准备 models/translation/m2m100_418m".to_string()
        })
}

fn contains_chinese(text: &str) -> bool {
    contains_cjk(text)
}

#[allow(dead_code)]
fn _subtitle_path_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
