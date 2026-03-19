use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
    pub position: OverlayPosition,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            font_size: 34.0,
            opacity: 1.0,
            color: "#FFFFFF".to_string(),
            position: OverlayPosition::Bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub playback_rate: f64,
    pub overlay_visible: bool,
    pub overlay: OverlaySettings,
    pub playlist_mode: String,
    /// The ID of the currently selected whisper model (e.g. "tiny", "base", "small", "medium", "large-v3-turbo").
    pub selected_model: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            playback_rate: 1.0,
            overlay_visible: false,
            overlay: OverlaySettings::default(),
            playlist_mode: "sequential".to_string(),
            selected_model: "base".to_string(),
        }
    }
}

pub struct AppState {
    pub settings: Arc<Mutex<AppSettings>>,
    pub active_asr_job: Arc<Mutex<Option<String>>>,
    pub active_model_download: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            active_asr_job: Arc::new(Mutex::new(None)),
            active_model_download: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWindowState {
    pub visible: bool,
}
