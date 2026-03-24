use crate::state::AppSettings;
use std::{fs, path::Path};
use tauri::{AppHandle, Manager};

const SETTINGS_FILE_NAME: &str = "settings.json";

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;

    if !path.exists() {
        let settings = AppSettings::default();
        save_settings(app, &settings)?;
        return Ok(settings);
    }

    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取设置文件失败: {error}"))?;

    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|error| format!("解析设置文件失败: {error}"))?;
    let has_completed_onboarding = value.get("hasCompletedOnboarding").is_some();
    let has_seen_main_tour = value.get("hasSeenMainTour").is_some();

    let mut settings: AppSettings =
        serde_json::from_value(value).map_err(|error| format!("解析设置内容失败: {error}"))?;

    // 兼容旧版本配置：旧用户升级后不应再次看到首次启动引导。
    if !has_completed_onboarding {
        settings.has_completed_onboarding = true;
    }
    if !has_seen_main_tour {
        settings.has_seen_main_tour = true;
    }

    Ok(settings)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    ensure_parent_dir(&path)?;

    let content = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("序列化设置失败: {error}"))?;

    fs::write(&path, content).map_err(|error| format!("写入设置文件失败: {error}"))
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("解析应用数据目录失败: {error}"))?;

    Ok(dir.join(SETTINGS_FILE_NAME))
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "设置文件目录不存在".to_string())?;

    fs::create_dir_all(parent).map_err(|error| format!("创建设置目录失败: {error}"))
}
