use crate::{media, model, state::AppSettings, store};
use serde::Serialize;
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub deleted_files: usize,
    pub deleted_dirs: usize,
}

pub fn clear_subtitles(app: &AppHandle) -> Result<CleanupResult, String> {
    let result = clear_relative_dir(app, "subtitles")?;
    media::clear_subtitle_references(app)?;
    Ok(result)
}

pub fn clear_audio_cache(app: &AppHandle) -> Result<CleanupResult, String> {
    clear_relative_dir(app, "cache/audio")
}

pub fn delete_default_model(app: &AppHandle) -> Result<CleanupResult, String> {
    let path = model::resolve_default_model_path(app)?;
    if !path.exists() {
        return Ok(CleanupResult {
            deleted_files: 0,
            deleted_dirs: 0,
        });
    }

    fs::remove_file(&path).map_err(|error| format!("删除默认模型失败: {error}"))?;
    prune_empty_parent_dirs(&path, app)?;

    Ok(CleanupResult {
        deleted_files: 1,
        deleted_dirs: 0,
    })
}

pub fn reset_app_data(app: &AppHandle) -> Result<CleanupResult, String> {
    let app_data_dir = app_data_dir(app)?;

    let mut result = CleanupResult {
        deleted_files: 0,
        deleted_dirs: 0,
    };

    for relative in ["subtitles", "cache/audio", "media"] {
        let cleanup = clear_relative_dir(app, relative)?;
        result.deleted_files += cleanup.deleted_files;
        result.deleted_dirs += cleanup.deleted_dirs;
    }

    let model_cleanup = delete_default_model(app)?;
    result.deleted_files += model_cleanup.deleted_files;
    result.deleted_dirs += model_cleanup.deleted_dirs;

    let settings_path = app_data_dir.join("settings.json");
    if settings_path.exists() {
        fs::remove_file(&settings_path).map_err(|error| format!("删除设置文件失败: {error}"))?;
        result.deleted_files += 1;
    }

    let library_path = app_data_dir.join("library.json");
    if library_path.exists() {
        fs::remove_file(&library_path).map_err(|error| format!("删除素材库失败: {error}"))?;
        result.deleted_files += 1;
    }

    store::save_settings(app, &AppSettings::default())?;

    Ok(result)
}

fn clear_relative_dir(app: &AppHandle, relative: &str) -> Result<CleanupResult, String> {
    let target_dir = app_data_dir(app)?.join(relative);
    if !target_dir.exists() {
        return Ok(CleanupResult {
            deleted_files: 0,
            deleted_dirs: 0,
        });
    }

    let mut result = CleanupResult {
        deleted_files: 0,
        deleted_dirs: 0,
    };

    clear_dir_contents(&target_dir, &mut result)?;

    Ok(result)
}

fn clear_dir_contents(dir: &PathBuf, result: &mut CleanupResult) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|error| format!("读取目录失败: {error}"))? {
        let entry = entry.map_err(|error| format!("读取目录项失败: {error}"))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|error| format!("读取文件信息失败: {error}"))?;

        if metadata.is_dir() {
            clear_dir_contents(&path, result)?;
            fs::remove_dir_all(&path).map_err(|error| format!("删除目录失败: {error}"))?;
            result.deleted_dirs += 1;
        } else {
            fs::remove_file(&path).map_err(|error| format!("删除文件失败: {error}"))?;
            result.deleted_files += 1;
        }
    }

    Ok(())
}

fn prune_empty_parent_dirs(path: &PathBuf, app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app_data_dir(app)?;
    let mut current = path.parent();

    while let Some(dir) = current {
        if dir == app_data_dir {
            break;
        }

        let is_empty = fs::read_dir(dir)
            .map_err(|error| format!("读取目录失败: {error}"))?
            .next()
            .is_none();

        if is_empty {
            fs::remove_dir(dir).map_err(|error| format!("删除空目录失败: {error}"))?;
        } else {
            break;
        }

        current = dir.parent();
    }

    Ok(())
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))
}
