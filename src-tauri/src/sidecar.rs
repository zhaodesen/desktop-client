use std::{
    env,
    path::PathBuf,
    process::Command,
};
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

    Err(format!(
        "未找到可用的可执行文件。请把二进制放到 src-tauri/binaries 并通过 externalBin 打包，或通过环境变量 {} 指定绝对路径。",
        env_key
    ))
}

pub fn resolve_local_candidates(app: &AppHandle, names: &[&str]) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    let current_dir = env::current_dir().map_err(|error| format!("读取当前目录失败: {error}"))?;

    for name in names {
        let binary_name = with_target_triple(name);
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

/// 构建低优先级命令：在 Unix 上使用 nice -n 15 降低调度优先级，
/// 避免 ffmpeg / whisper-cli 等 CPU 密集型子进程抢占 UI 线程。
/// Windows 上退化为普通 `build_command`（Windows 进程优先级需要另外处理）。
pub fn build_nice_command(target: &CommandTarget) -> Command {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("nice");
        cmd.args(["-n", "15"]);
        match target {
            CommandTarget::Program(program) => { cmd.arg(program); }
            CommandTarget::File(path) => { cmd.arg(path); }
        }
        cmd
    }
    #[cfg(not(unix))]
    {
        build_command(target)
    }
}
