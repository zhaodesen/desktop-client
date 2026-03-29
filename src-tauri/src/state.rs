use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Child,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayPosition {
    Top,
    Bottom,
}

impl Default for OverlayPosition {
    fn default() -> Self {
        Self::Bottom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlaySettings {
    pub font_size: f64,
    pub opacity: f64,
    pub color: String,
    pub stroke_color: String,
    pub secondary_color: String,
    pub secondary_stroke_color: String,
    pub position: OverlayPosition,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            font_size: 34.0,
            opacity: 1.0,
            color: "#FFFFFF".to_string(),
            stroke_color: "#000000".to_string(),
            secondary_color: "#FFFFFF".to_string(),
            secondary_stroke_color: "#000000".to_string(),
            position: OverlayPosition::Bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleDisplayMode {
    Original,
    Translation,
    Bilingual,
}

impl Default for SubtitleDisplayMode {
    fn default() -> Self {
        Self::Bilingual
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    pub play_pause: String,
    pub previous_track: String,
    pub next_track: String,
    pub toggle_overlay: String,
    pub volume_up: String,
    pub volume_down: String,
    pub show_translation: String,
    pub show_original: String,
    pub show_bilingual: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            play_pause: "Space".to_string(),
            previous_track: "Comma".to_string(),
            next_track: "Period".to_string(),
            toggle_overlay: "KeyO".to_string(),
            volume_up: "Equal".to_string(),
            volume_down: "Minus".to_string(),
            show_translation: "Digit1".to_string(),
            show_original: "Digit2".to_string(),
            show_bilingual: "Digit3".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybackState {
    pub media_id: String,
    pub current_time_ms: u64,
    pub was_playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub playback_rate: f64,
    pub volume: f64,
    pub overlay_visible: bool,
    pub overlay: OverlaySettings,
    pub playlist_mode: String,
    pub subtitle_display_mode: SubtitleDisplayMode,
    pub shortcuts: ShortcutSettings,
    /// The ID of the currently selected whisper model (e.g. "tiny", "base", "small", "medium", "large-v3-turbo").
    pub selected_model: String,
    pub has_completed_onboarding: bool,
    pub has_seen_main_tour: bool,
    /// UI theme: "dark", "light", or "system"
    pub theme_mode: String,
    pub playback_state: Option<PlaybackState>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            playback_rate: 1.0,
            volume: 1.0,
            overlay_visible: false,
            overlay: OverlaySettings::default(),
            playlist_mode: "sequential".to_string(),
            subtitle_display_mode: SubtitleDisplayMode::Bilingual,
            shortcuts: ShortcutSettings::default(),
            selected_model: "base".to_string(),
            has_completed_onboarding: false,
            has_seen_main_tour: false,
            theme_mode: "dark".to_string(),
            playback_state: None,
        }
    }
}

pub struct AppState {
    pub settings: Arc<Mutex<AppSettings>>,
    pub active_asr_job: Arc<Mutex<Option<AsrJobState>>>,
    pub active_model_download: Arc<Mutex<Option<ModelDownloadState>>>,
    pub active_online_import: Arc<Mutex<Option<ExternalProcessState>>>,
    pub active_translation_job: Arc<Mutex<Option<ExternalProcessState>>>,
    pub shutdown_confirmed: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            active_asr_job: Arc::new(Mutex::new(None)),
            active_model_download: Arc::new(Mutex::new(None)),
            active_online_import: Arc::new(Mutex::new(None)),
            active_translation_job: Arc::new(Mutex::new(None)),
            shutdown_confirmed: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone)]
pub struct AsrJobState {
    pub job_id: String,
    pub cancel_requested: Arc<AtomicBool>,
    pub active_child: Arc<Mutex<Option<Child>>>,
}

impl AsrJobState {
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            active_child: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Clone)]
pub struct ModelDownloadState {
    pub job_id: String,
    pub cancel_requested: Arc<AtomicBool>,
    pub pause_requested: Arc<AtomicBool>,
    pub temp_path: Arc<Mutex<Option<PathBuf>>>,
}

impl ModelDownloadState {
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            pause_requested: Arc::new(AtomicBool::new(false)),
            temp_path: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Clone)]
pub struct ExternalProcessState {
    pub label: &'static str,
    pub pid: u32,
}

impl ExternalProcessState {
    pub fn new(label: &'static str, pid: u32) -> Self {
        Self { label, pid }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWindowState {
    pub visible: bool,
}
