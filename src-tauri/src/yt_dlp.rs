use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering as CmpOrdering,
    env,
    fs::{self, File},
    io::copy,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;

use crate::sidecar::{self, CommandTarget};

const YT_DLP_NAME: &str = "yt-dlp";
const AUTO_UPDATE_INTERVAL_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const RETRY_INTERVAL_MS: u64 = 12 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpStatus {
    pub current_version: Option<String>,
    pub bundled_version: Option<String>,
    pub override_version: Option<String>,
    pub current_path: Option<String>,
    pub source: String,
    pub last_checked_at: Option<u64>,
    pub last_updated_at: Option<u64>,
    pub last_error: Option<String>,
    pub update_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct YtDlpUpdateState {
    last_checked_at: Option<u64>,
    last_updated_at: Option<u64>,
    last_error: Option<String>,
}

pub fn maybe_auto_update(app: &AppHandle) -> Result<YtDlpStatus, String> {
    refresh_impl(app, false).or_else(|error| {
        eprintln!("yt-dlp 自动更新检查失败: {error}");
        get_status(app)
    })
}

pub fn force_update(app: &AppHandle) -> Result<YtDlpStatus, String> {
    refresh_impl(app, true)
}

pub fn get_status(app: &AppHandle) -> Result<YtDlpStatus, String> {
    let state = load_update_state(app).unwrap_or_default();
    let env_override = resolve_env_override();
    let override_path = override_binary_path(app).ok();
    let bundled_path = find_bundled_binary(app);
    let current_target = sidecar::locate_executable(app, "YT_DLP_BIN", &[YT_DLP_NAME]).ok();

    let source = if env_override.is_some() {
        "env".to_string()
    } else if let (Some(path), Some(current_path)) = (
        override_path.as_ref(),
        current_target.as_ref().and_then(command_target_path),
    ) {
        if current_path == path {
            "appData".to_string()
        } else if bundled_path.as_ref().is_some_and(|bundled| bundled == current_path) {
            "bundle".to_string()
        } else {
            "path".to_string()
        }
    } else if let Some(current_path) = current_target.as_ref().and_then(command_target_path) {
        if bundled_path.as_ref().is_some_and(|bundled| bundled == current_path) {
            "bundle".to_string()
        } else {
            "path".to_string()
        }
    } else {
        "missing".to_string()
    };

    Ok(YtDlpStatus {
        current_version: current_target
            .as_ref()
            .and_then(read_version_from_target),
        bundled_version: bundled_path
            .as_ref()
            .map(|path| CommandTarget::File(path.clone()))
            .as_ref()
            .and_then(read_version_from_target),
        override_version: override_path
            .as_ref()
            .filter(|path| path.exists())
            .map(|path| CommandTarget::File(path.clone()))
            .as_ref()
            .and_then(read_version_from_target),
        current_path: current_target
            .as_ref()
            .map(|target| target.display()),
        source,
        last_checked_at: state.last_checked_at,
        last_updated_at: state.last_updated_at,
        last_error: state.last_error,
        update_supported: env_override.is_none() && download_url_for_current_platform().is_some(),
    })
}

fn refresh_impl(app: &AppHandle, force: bool) -> Result<YtDlpStatus, String> {
    let mut state = load_update_state(app).unwrap_or_default();
    let status = get_status(app)?;
    if !status.update_supported {
        return Ok(status);
    }

    let now = now_millis();
    if !force && !should_check(&state, now) {
        return Ok(status);
    }

    state.last_checked_at = Some(now);
    save_update_state(app, &state)?;

    let current_version = status.current_version.clone();
    match download_latest_binary(app, current_version.as_deref()) {
        Ok(Some(_)) => {
            state.last_updated_at = Some(now);
            state.last_error = None;
            save_update_state(app, &state)?;
        }
        Ok(None) => {
            state.last_error = None;
            save_update_state(app, &state)?;
        }
        Err(error) => {
            state.last_error = Some(error.clone());
            save_update_state(app, &state)?;
            if force {
                return Err(error);
            }
        }
    }

    get_status(app)
}

fn should_check(state: &YtDlpUpdateState, now: u64) -> bool {
    let Some(last_checked_at) = state.last_checked_at else {
        return true;
    };
    let interval = if state.last_error.is_some() {
        RETRY_INTERVAL_MS
    } else {
        AUTO_UPDATE_INTERVAL_MS
    };
    now.saturating_sub(last_checked_at) >= interval
}

fn download_latest_binary(
    app: &AppHandle,
    current_version: Option<&str>,
) -> Result<Option<String>, String> {
    let download_url = download_url_for_current_platform()
        .ok_or_else(|| "当前平台暂不支持 yt-dlp 自动更新".to_string())?;
    let sidecar_dir = sidecar::app_data_sidecar_dir(app)?;
    fs::create_dir_all(&sidecar_dir).map_err(|error| format!("创建 sidecar 目录失败: {error}"))?;

    let target_path = override_binary_path(app)?;
    let temp_path = target_path.with_extension("download");
    let client = Client::builder()
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("创建 yt-dlp 更新客户端失败: {error}"))?;

    let mut response = client
        .get(&download_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("下载最新 yt-dlp 失败: {error}"))?;
    let mut output = File::create(&temp_path).map_err(|error| format!("创建临时文件失败: {error}"))?;
    copy(&mut response, &mut output).map_err(|error| format!("写入 yt-dlp 更新文件失败: {error}"))?;
    drop(output);

    ensure_executable(&temp_path)?;

    let candidate_target = CommandTarget::File(temp_path.clone());
    let candidate_version = read_version_from_target(&candidate_target)
        .ok_or_else(|| "最新 yt-dlp 下载成功，但无法读取版本号".to_string())?;

    let should_promote = current_version
        .map(|version| compare_versions(&candidate_version, version) == CmpOrdering::Greater)
        .unwrap_or(true);

    if !should_promote {
        let _ = fs::remove_file(&temp_path);
        return Ok(None);
    }

    if target_path.exists() {
        fs::remove_file(&target_path).map_err(|error| format!("替换旧版 yt-dlp 失败: {error}"))?;
    }
    fs::rename(&temp_path, &target_path).map_err(|error| format!("启用新版 yt-dlp 失败: {error}"))?;
    ensure_executable(&target_path)?;
    sign_binary_for_macos(&target_path)?;

    Ok(Some(candidate_version))
}

fn compare_versions(left: &str, right: &str) -> CmpOrdering {
    let parse = |value: &str| {
        value.split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };

    let left = parse(left);
    let right = parse(right);
    let len = left.len().max(right.len());

    for index in 0..len {
        let l = *left.get(index).unwrap_or(&0);
        let r = *right.get(index).unwrap_or(&0);
        match l.cmp(&r) {
            CmpOrdering::Equal => {}
            ordering => return ordering,
        }
    }

    CmpOrdering::Equal
}

fn read_version_from_target(target: &CommandTarget) -> Option<String> {
    let output = sidecar::build_command(target)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

fn command_target_path(target: &CommandTarget) -> Option<&PathBuf> {
    match target {
        CommandTarget::Program(_) => None,
        CommandTarget::File(path) => Some(path),
    }
}

fn override_binary_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(sidecar::app_data_sidecar_dir(app)?.join(sidecar::with_target_triple(YT_DLP_NAME)))
}

fn find_bundled_binary(app: &AppHandle) -> Option<PathBuf> {
    sidecar::resolve_local_candidates(app, &[YT_DLP_NAME])
        .ok()?
        .into_iter()
        .find(|path| path.exists())
}

fn load_update_state(app: &AppHandle) -> Result<YtDlpUpdateState, String> {
    let path = update_state_path(app)?;
    if !path.exists() {
        return Ok(YtDlpUpdateState::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|error| format!("读取 yt-dlp 状态失败: {error}"))?;
    serde_json::from_str(&content).map_err(|error| format!("解析 yt-dlp 状态失败: {error}"))
}

fn save_update_state(app: &AppHandle, state: &YtDlpUpdateState) -> Result<(), String> {
    let path = update_state_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 yt-dlp 状态目录失败: {error}"))?;
    }

    let content =
        serde_json::to_string_pretty(state).map_err(|error| format!("序列化 yt-dlp 状态失败: {error}"))?;
    fs::write(&path, content).map_err(|error| format!("写入 yt-dlp 状态失败: {error}"))
}

fn update_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(sidecar::app_data_sidecar_dir(app)?.join("yt-dlp-state.json"))
}

fn resolve_env_override() -> Option<String> {
    std::env::var("YT_DLP_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn ensure_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("读取 yt-dlp 权限失败: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("设置 yt-dlp 可执行权限失败: {error}"))?;
    }

    Ok(())
}

fn sign_binary_for_macos(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let entitlements_path = env::temp_dir().join(format!(
            "muyu-yt-dlp-entitlements-{}.plist",
            std::process::id()
        ));
        let entitlements = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.cs.disable-library-validation</key>
  <true/>
</dict>
</plist>
"#;

        fs::write(&entitlements_path, entitlements)
            .map_err(|error| format!("写入 yt-dlp entitlement 失败: {error}"))?;

        let result = Command::new("codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--options",
                "runtime",
                "--entitlements",
            ])
            .arg(&entitlements_path)
            .arg(path)
            .status()
            .map_err(|error| format!("执行 yt-dlp 签名失败: {error}"))?;

        let _ = fs::remove_file(&entitlements_path);

        if !result.success() {
            return Err(format!("签名新版 yt-dlp 失败，退出码: {:?}", result.code()));
        }
    }

    Ok(())
}

fn download_url_for_current_platform() -> Option<String> {
    if let Some(custom) = std::env::var("MUYU_YT_DLP_DOWNLOAD_URL")
        .ok()
        .or_else(|| std::env::var("YT_DLP_DOWNLOAD_URL").ok())
    {
        let custom = custom.trim().to_string();
        if !custom.is_empty() {
            return Some(custom);
        }
    }

    #[cfg(target_os = "macos")]
    {
        Some("https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        Some("https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}
