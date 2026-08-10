use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tauri::{path::BaseDirectory, AppHandle, Manager};

#[cfg(windows)]
use std::{ffi::OsString, os::windows::process::CommandExt};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub enum CommandTarget {
    Program(String),
    File(PathBuf),
}

impl CommandTarget {
    pub fn display(&self) -> String {
        match self {
            Self::Program(program) => program.clone(),
            Self::File(path) => path.display().to_string(),
        }
    }
}

pub fn locate_executable(
    app: &AppHandle,
    env_key: &str,
    file_candidates: &[&str],
) -> Result<CommandTarget, String> {
    if let Ok(value) = env::var(env_key) {
        let path = PathBuf::from(&value);
        if path.exists() {
            return Ok(CommandTarget::File(path));
        }
        return Ok(CommandTarget::Program(value));
    }

    for candidate in resolve_app_data_candidates(app, file_candidates)? {
        if candidate.exists() {
            return Ok(CommandTarget::File(candidate));
        }
    }

    for candidate in resolve_local_candidates(app, file_candidates)? {
        if candidate.exists() {
            return Ok(CommandTarget::File(candidate));
        }
    }

    if let Some(candidate) = resolve_path_candidate(file_candidates) {
        return Ok(CommandTarget::File(candidate));
    }

    Err(format!(
        "未找到可用的可执行文件。请将二进制放到 src-tauri/binaries、安装到系统 PATH，或通过环境变量 {env_key} 指定绝对路径。"
    ))
}

pub fn app_data_sidecar_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败: {error}"))?
        .join("sidecars"))
}

pub fn resolve_local_candidates(app: &AppHandle, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;
    let mut candidates = Vec::new();

    for name in names {
        let binary_name = with_target_triple(name);
        let exe_name = with_exe_suffix(name);

        // In dev, prefer the checked-in sidecars over target/debug copies. The
        // checked-in binaries live next to their DLLs and are safe to launch.
        for dir in local_sidecar_dirs(&current_dir) {
            push_unique_path(&mut candidates, dir.join(&binary_name));
            push_unique_path(&mut candidates, dir.join(&exe_name));
            push_matching_sidecar_paths(&mut candidates, &dir, name);
        }

        if let Ok(resource_path) = app.path().resolve(&binary_name, BaseDirectory::Resource) {
            push_unique_path(&mut candidates, resource_path);
        }
        if let Ok(resource_path) = app.path().resolve(&exe_name, BaseDirectory::Resource) {
            push_unique_path(&mut candidates, resource_path);
        }
        if let Ok(resource_path) = app
            .path()
            .resolve(format!("binaries/{binary_name}"), BaseDirectory::Resource)
        {
            push_unique_path(&mut candidates, resource_path);
        }
        if let Ok(resource_path) = app
            .path()
            .resolve(format!("binaries/{exe_name}"), BaseDirectory::Resource)
        {
            push_unique_path(&mut candidates, resource_path);
        }
        if let Ok(resource_dir) = app.path().resolve(".", BaseDirectory::Resource) {
            push_matching_sidecar_paths(&mut candidates, &resource_dir, name);
        }
        if let Ok(resource_dir) = app.path().resolve("binaries", BaseDirectory::Resource) {
            push_matching_sidecar_paths(&mut candidates, &resource_dir, name);
        }
        if let Ok(executable_path) = app.path().resolve(&binary_name, BaseDirectory::Executable) {
            push_unique_path(&mut candidates, executable_path);
        }
        if let Ok(executable_path) = app.path().resolve(&exe_name, BaseDirectory::Executable) {
            push_unique_path(&mut candidates, executable_path);
        }
        if let Ok(executable_dir) = app.path().resolve(".", BaseDirectory::Executable) {
            push_matching_sidecar_paths(&mut candidates, &executable_dir, name);
        }
        for executable_dir in current_executable_dirs() {
            push_unique_path(&mut candidates, executable_dir.join(&binary_name));
            push_unique_path(&mut candidates, executable_dir.join(&exe_name));
            push_matching_sidecar_paths(&mut candidates, &executable_dir, name);
        }
    }

    Ok(candidates)
}

fn resolve_app_data_candidates(app: &AppHandle, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let dir = app_data_sidecar_dir(app)?;
    let mut candidates = Vec::new();

    for name in names {
        push_unique_path(&mut candidates, dir.join(with_target_triple(name)));
        push_unique_path(&mut candidates, dir.join(with_exe_suffix(name)));
        push_matching_sidecar_paths(&mut candidates, &dir, name);
    }

    Ok(candidates)
}

fn resolve_path_candidate(names: &[&str]) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        for name in names {
            let candidate = dir.join(with_exe_suffix(name));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn with_target_triple(name: &str) -> String {
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

pub fn with_exe_suffix(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[allow(dead_code)]
pub fn build_command(target: &CommandTarget) -> Command {
    #[cfg(windows)]
    let mut command = match target {
        CommandTarget::Program(program) => Command::new(program),
        CommandTarget::File(path) => Command::new(path),
    };

    #[cfg(not(windows))]
    let command = match target {
        CommandTarget::Program(program) => Command::new(program),
        CommandTarget::File(path) => Command::new(path),
    };

    #[cfg(windows)]
    configure_windows_command(&mut command, target);

    command
}

pub fn build_nice_command(target: &CommandTarget) -> Command {
    #[cfg(target_os = "macos")]
    {
        // 直接启动子进程，避免依赖 taskpolicy 包装器。
        // 线上已出现 "taskpolicy: posix_spawn: Permission denied"，
        // 但同一可执行文件在终端中可正常被 taskpolicy 启动，说明问题出在
        // app 运行时上下文下的二次 spawn，而不是 sidecar 本身不可执行。
        build_command(target)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut cmd = Command::new("nice");
        cmd.args(["-n", "19"]);
        match target {
            CommandTarget::Program(program) => {
                cmd.arg(program);
            }
            CommandTarget::File(path) => {
                cmd.arg(path);
            }
        }
        cmd
    }
    #[cfg(not(unix))]
    {
        build_command(target)
    }
}

pub fn spawn_command_with_priority<F>(target: &CommandTarget, configure: F) -> Result<Child, io::Error>
where
    F: Fn(&mut Command),
{
    let mut command = build_nice_command(target);
    configure(&mut command);
    command.spawn()
}

pub fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|error| format!("终止进程失败: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("终止进程失败，退出码: {:?}", status.code()))
        }
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|error| format!("终止进程失败: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("终止进程失败，退出码: {:?}", status.code()))
        }
    }
}

fn local_sidecar_dirs(current_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        current_dir.join("src-tauri/binaries"),
        current_dir.join("bin"),
    ];

    if let Some(parent_dir) = current_dir.parent() {
        dirs.push(parent_dir.join("src-tauri/binaries"));
        dirs.push(parent_dir.join("bin"));
    }

    dirs
}

fn current_executable_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let Ok(current_exe) = env::current_exe() else {
        return dirs;
    };

    if let Some(parent) = current_exe.parent() {
        push_unique_path(&mut dirs, parent.to_path_buf());

        if cfg!(target_os = "macos") {
            if let Some(contents_dir) = parent.parent() {
                push_unique_path(&mut dirs, contents_dir.join("Resources"));
                push_unique_path(&mut dirs, contents_dir.join("MacOS"));
            }
        }
    }

    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|path| path == &candidate) {
        paths.push(candidate);
    }
}

fn push_matching_sidecar_paths(paths: &mut Vec<PathBuf>, dir: &Path, name: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        if is_matching_sidecar_path(&path, file_name, name) {
            push_unique_path(paths, path);
        }
    }
}

fn is_matching_sidecar_path(path: &Path, file_name: &str, base_name: &str) -> bool {
    if !is_matching_sidecar_name(file_name, base_name) {
        return false;
    }

    #[cfg(unix)]
    {
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        let _ = path;
        true
    }
}

fn is_matching_sidecar_name(file_name: &str, base_name: &str) -> bool {
    if cfg!(windows) {
        let lowered = file_name.to_ascii_lowercase();
        let exact = format!("{base_name}.exe");
        let prefixed = format!("{base_name}-");
        lowered == exact || (lowered.starts_with(&prefixed) && lowered.ends_with(".exe"))
    } else {
        let has_extension = Path::new(file_name).extension().is_some();
        !has_extension && (file_name == base_name || file_name.starts_with(&format!("{base_name}-")))
    }
}

#[cfg(test)]
mod tests {
    use super::{is_matching_sidecar_name, is_matching_sidecar_path};
    use std::{fs, path::PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn sidecar_name_does_not_match_state_json() {
        assert!(!is_matching_sidecar_name("model-state.json", "model"));
        assert!(!is_matching_sidecar_name("translator-cli-state.json", "translator-cli"));
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_path_requires_executable_bit_on_unix() {
        let temp_dir = std::env::temp_dir().join(format!("muyu-sidecar-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let executable_path = temp_dir.join("model");
        fs::write(&executable_path, b"#!/bin/sh\n").expect("write executable");
        let mut executable_permissions = fs::metadata(&executable_path)
            .expect("read executable metadata")
            .permissions();
        executable_permissions.set_mode(0o755);
        fs::set_permissions(&executable_path, executable_permissions).expect("chmod executable");

        let state_path = temp_dir.join("model-state.json");
        fs::write(&state_path, b"{}").expect("write state file");
        let mut state_permissions = fs::metadata(&state_path)
            .expect("read state metadata")
            .permissions();
        state_permissions.set_mode(0o644);
        fs::set_permissions(&state_path, state_permissions).expect("chmod state");

        assert!(is_matching_sidecar_path(&executable_path, "model", "model"));
        assert!(!is_matching_sidecar_path(
            &PathBuf::from(&state_path),
            "model-state.json",
            "model",
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(windows)]
fn configure_windows_command(command: &mut Command, target: &CommandTarget) {
    command.creation_flags(CREATE_NO_WINDOW);

    let executable_path = match target {
        CommandTarget::Program(_) => return,
        CommandTarget::File(path) => path,
    };

    if let Some(parent) = executable_path.parent() {
        command.current_dir(parent);
    }

    let mut path_entries = Vec::new();

    if let Some(parent) = executable_path.parent() {
        push_unique_os_path(&mut path_entries, parent.to_path_buf());
    }

    if let Ok(current_dir) = env::current_dir() {
        for dir in local_sidecar_dirs(&current_dir) {
            if dir.is_dir() {
                push_unique_os_path(&mut path_entries, dir);
            }
        }
    }

    if let Some(existing_path) = env::var_os("PATH") {
        for entry in env::split_paths(&existing_path) {
            push_unique_os_path(&mut path_entries, entry);
        }
    }

    if let Ok(joined) = env::join_paths(path_entries) {
        command.env("PATH", joined);
    }
}

#[cfg(windows)]
fn push_unique_os_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidate.exists() {
        return;
    }

    if !paths.iter().any(|path| same_windows_path(path, &candidate)) {
        paths.push(candidate);
    }
}

#[cfg(windows)]
fn same_windows_path(left: &Path, right: &Path) -> bool {
    normalize_windows_path(left) == normalize_windows_path(right)
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> OsString {
    OsString::from(path.as_os_str().to_string_lossy().to_ascii_lowercase())
}
