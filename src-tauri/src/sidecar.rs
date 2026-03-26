use std::{env, path::PathBuf, process::Command};
use tauri::{path::BaseDirectory, AppHandle, Manager};

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

    for candidate in resolve_local_candidates(app, file_candidates)? {
        if candidate.exists() {
            return Ok(CommandTarget::File(candidate));
        }
    }

    if let Some(candidate) = resolve_path_candidate(file_candidates) {
        return Ok(CommandTarget::File(candidate));
    }

    Err(format!(
        "未找到可用的可执行文件。请把二进制放到 src-tauri/binaries、安装到系统 PATH，或通过环境变量 {} 指定绝对路径。",
        env_key
    ))
}

pub fn resolve_local_candidates(app: &AppHandle, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;

    for name in names {
        let binary_name = with_target_triple(name);
        if let Ok(resource_path) = app.path().resolve(&binary_name, BaseDirectory::Resource) {
            candidates.push(resource_path);
        }
        if let Ok(resource_path) =
            app.path().resolve(with_exe_suffix(name), BaseDirectory::Resource)
        {
            candidates.push(resource_path);
        }
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
        if let Ok(executable_path) = app.path().resolve(&binary_name, BaseDirectory::Executable) {
            candidates.push(executable_path);
        }
        if let Ok(executable_path) =
            app.path().resolve(with_exe_suffix(name), BaseDirectory::Executable)
        {
            candidates.push(executable_path);
        }
        candidates.push(current_dir.join("src-tauri/binaries").join(&binary_name));
        candidates.push(
            current_dir
                .join("src-tauri/binaries")
                .join(with_exe_suffix(name)),
        );
        if let Some(parent_dir) = current_dir.parent() {
            candidates.push(parent_dir.join("src-tauri/binaries").join(&binary_name));
            candidates.push(
                parent_dir
                    .join("src-tauri/binaries")
                    .join(with_exe_suffix(name)),
            );
        }
        candidates.push(current_dir.join("bin").join(with_exe_suffix(name)));
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
    match target {
        CommandTarget::Program(program) => Command::new(program),
        CommandTarget::File(path) => Command::new(path),
    }
}

/// 构建低优先级命令，避免 ffmpeg / whisper-cli 等 CPU 密集型子进程抢占 UI 线程。
///
/// - **macOS**: 使用 `taskpolicy -b` 设置 TASK_BACKGROUND_APPLICATION QoS。
///   这是 XNU 内核级别的调度限制，比 `nice` 有效得多——macOS 几乎忽略 nice 值，
///   但 `taskpolicy -b` 会让进程真正退让 CPU 给前台应用（WKWebView）。
/// - **Linux**: 使用 `nice -n 19`（最低优先级），CFS 调度器会正确遵守。
/// - **Windows**: 退化为普通命令（未来可用 `START /LOW`）。
pub fn build_nice_command(target: &CommandTarget) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("taskpolicy");
        cmd.arg("-b");
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
