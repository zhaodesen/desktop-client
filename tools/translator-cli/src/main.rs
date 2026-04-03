use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const TARGET_LANGUAGE: &str = "zh";
const DEFAULT_MODEL_LAYOUT_NAME: &str = "m2m100_418m";
const MODEL_DIR_ENV: &str = "MUYU_TRANSLATION_MODEL_DIR";
const CT2_TRANSLATOR_BIN_ENV: &str = "CT2_TRANSLATOR_BIN";
const SPM_ENCODE_BIN_ENV: &str = "SPM_ENCODE_BIN";
const SPM_DECODE_BIN_ENV: &str = "SPM_DECODE_BIN";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranslationRequest {
    source_language: Option<String>,
    target_language: String,
    lines: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslationResponse {
    source_language: String,
    translations: Vec<String>,
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Result<Self, String> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("读取系统时间失败: {error}"))?
            .as_millis();
        let path = env::temp_dir().join(format!("{prefix}-{millis}-{}", std::process::id()));
        fs::create_dir_all(&path).map_err(|error| format!("创建临时目录失败: {error}"))?;
        Ok(Self { path })
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match run() {
        Ok(response) => {
            if let Err(error) = serde_json::to_writer(std::io::stdout(), &response) {
                eprintln!("输出翻译结果失败: {error}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_help() {
    println!(
        "\
translator-cli

用法:
  echo '{{\"sourceLanguage\":\"en\",\"targetLanguage\":\"zh\",\"lines\":[\"hello\"]}}' | translator-cli

环境变量:
  {MODEL_DIR_ENV}     指向翻译模型目录，默认查找 models/translation/{DEFAULT_MODEL_LAYOUT_NAME}
  {CT2_TRANSLATOR_BIN_ENV} 指向 ct2-translator 可执行文件
  {SPM_ENCODE_BIN_ENV}     指向 spm_encode 可执行文件
  {SPM_DECODE_BIN_ENV}     指向 spm_decode 可执行文件

模型目录约定:
  {DEFAULT_MODEL_LAYOUT_NAME}/
    ctranslate2/
    sentencepiece.bpe.model
"
    );
}

fn run() -> Result<TranslationResponse, String> {
    let request = load_request()?;
    let target_language = normalize_language_code(&request.target_language)?;
    if target_language != TARGET_LANGUAGE {
        return Err("原生离线翻译当前仅支持输出中文（zh）".to_string());
    }

    let source_language = match request.source_language.as_deref() {
        Some(code) => normalize_language_code(code)?,
        None => detect_source_language(&request.lines),
    };

    if source_language == target_language {
        return Ok(TranslationResponse {
            source_language,
            translations: request.lines,
        });
    }

    if !matches!(source_language.as_str(), "en" | "ja") {
        return Err(format!(
            "当前原生离线翻译仅支持 en/ja -> zh，收到源语言: {source_language}"
        ));
    }

    let model_root = resolve_model_root()?;
    let ct2_model_dir = resolve_ct2_model_dir(&model_root)?;
    let spm_model_path = resolve_sentencepiece_model(&model_root)?;
    let ct2_translator = resolve_helper_binary(CT2_TRANSLATOR_BIN_ENV, "ct2-translator")?;
    let spm_encode = resolve_helper_binary(SPM_ENCODE_BIN_ENV, "spm_encode")?;
    let spm_decode = resolve_helper_binary(SPM_DECODE_BIN_ENV, "spm_decode")?;

    let mut translations = vec![String::new(); request.lines.len()];
    let active_lines = request
        .lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                translations[index] = String::new();
                None
            } else {
                Some((index, trimmed.to_string()))
            }
        })
        .collect::<Vec<_>>();

    if active_lines.is_empty() {
        return Ok(TranslationResponse {
            source_language,
            translations,
        });
    }

    let active_texts = active_lines
        .iter()
        .map(|(_, line)| line.clone())
        .collect::<Vec<_>>();
    let encoded_lines = encode_lines(&spm_encode, &spm_model_path, &active_texts)?;
    if encoded_lines.len() != active_lines.len() {
        return Err("spm_encode 输出行数与输入不一致".to_string());
    }

    let source_lang_token = language_token(&source_language);
    let target_lang_token = language_token(&target_language);
    let source_token_lines = encoded_lines
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                format!("{source_lang_token} </s>")
            } else {
                format!("{source_lang_token} {line} </s>")
            }
        })
        .collect::<Vec<_>>();
    let target_prefix_lines = vec![target_lang_token.to_string(); active_lines.len()];

    let translated_token_lines = run_ct2_translator(
        &ct2_translator,
        &ct2_model_dir,
        &source_token_lines,
        &target_prefix_lines,
    )?;
    if translated_token_lines.len() != active_lines.len() {
        return Err("ct2-translator 输出行数与输入不一致".to_string());
    }

    let cleaned_token_lines = translated_token_lines
        .into_iter()
        .map(|line| strip_target_prefix_and_eos(&line, target_lang_token))
        .collect::<Vec<_>>();
    let decoded_lines = decode_lines(&spm_decode, &spm_model_path, &cleaned_token_lines)?;
    if decoded_lines.len() != active_lines.len() {
        return Err("spm_decode 输出行数与输入不一致".to_string());
    }

    for ((index, _), translated) in active_lines.into_iter().zip(decoded_lines) {
        translations[index] = translated.trim().to_string();
    }

    Ok(TranslationResponse {
        source_language,
        translations,
    })
}

fn load_request() -> Result<TranslationRequest, String> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| format!("读取翻译请求失败: {error}"))?;
    if raw.trim().is_empty() {
        return Err("翻译请求为空".to_string());
    }
    serde_json::from_str(&raw).map_err(|error| format!("解析翻译请求失败: {error}"))
}

fn normalize_language_code(code: &str) -> Result<String, String> {
    let lowered = code.trim().to_ascii_lowercase();
    if lowered.is_empty() || lowered == "auto" {
        return Err("语言代码不能为空或 auto".to_string());
    }
    if lowered.starts_with("zh") {
        return Ok("zh".to_string());
    }
    if lowered.starts_with("en") {
        return Ok("en".to_string());
    }
    if lowered.starts_with("ja") || lowered.starts_with("jp") || lowered.starts_with("jpn") {
        return Ok("ja".to_string());
    }
    Err(format!("不支持的语言代码: {code}"))
}

fn detect_source_language(lines: &[String]) -> String {
    let sample = lines
        .iter()
        .map(String::as_str)
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    if contains_japanese(sample) {
        return "ja".to_string();
    }
    if contains_chinese(sample) {
        return "zh".to_string();
    }
    "en".to_string()
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|ch| {
        ('\u{3040}'..='\u{309f}').contains(&ch)
            || ('\u{30a0}'..='\u{30ff}').contains(&ch)
            || ('\u{31f0}'..='\u{31ff}').contains(&ch)
    })
}

fn contains_chinese(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn language_token(language: &str) -> &'static str {
    match language {
        "en" => "__en__",
        "ja" => "__ja__",
        "zh" => "__zh__",
        _ => "__en__",
    }
}

fn resolve_model_root() -> Result<PathBuf, String> {
    if let Ok(value) = env::var(MODEL_DIR_ENV) {
        let path = PathBuf::from(value);
        if path.exists() {
            return Ok(path);
        }
    }

    let current_exe =
        env::current_exe().map_err(|error| format!("读取当前可执行路径失败: {error}"))?;
    let executable_dir = current_exe
        .parent()
        .ok_or_else(|| "无法定位 translator-cli 所在目录".to_string())?;
    let mut candidates = vec![
        executable_dir
            .join("../Resources/models/translation")
            .join(DEFAULT_MODEL_LAYOUT_NAME),
        executable_dir.join("../Resources/models/translation"),
        executable_dir
            .join("models/translation")
            .join(DEFAULT_MODEL_LAYOUT_NAME),
        executable_dir.join("models/translation"),
    ];

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(
            current_dir
                .join("models/translation")
                .join(DEFAULT_MODEL_LAYOUT_NAME),
        );
        candidates.push(current_dir.join("models/translation"));
    }

    candidates.into_iter().find(|path| path.exists()).ok_or_else(|| {
        "未找到翻译模型目录，请设置 MUYU_TRANSLATION_MODEL_DIR 或准备 models/translation/m2m100_418m"
            .to_string()
    })
}

fn resolve_ct2_model_dir(model_root: &Path) -> Result<PathBuf, String> {
    let nested = model_root.join("ctranslate2");
    if nested.join("model.bin").exists() {
        return Ok(nested);
    }
    if model_root.join("model.bin").exists() {
        return Ok(model_root.to_path_buf());
    }
    Err(format!(
        "未找到 CTranslate2 模型目录，期望存在 {}/ctranslate2/model.bin 或 {}/model.bin",
        model_root.display(),
        model_root.display()
    ))
}

fn resolve_sentencepiece_model(model_root: &Path) -> Result<PathBuf, String> {
    for candidate in [
        model_root.join("sentencepiece.bpe.model"),
        model_root.join("spm.128k.model"),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "未找到 SentencePiece 模型文件，期望存在 {}/sentencepiece.bpe.model",
        model_root.display()
    ))
}

fn resolve_helper_binary(env_key: &str, base_name: &str) -> Result<PathBuf, String> {
    if let Ok(value) = env::var(env_key) {
        let path = PathBuf::from(&value);
        if path.exists() {
            return Ok(path);
        }
    }

    let current_exe =
        env::current_exe().map_err(|error| format!("读取当前可执行路径失败: {error}"))?;
    let executable_dir = current_exe
        .parent()
        .ok_or_else(|| "无法定位 translator-cli 所在目录".to_string())?;
    if let Some(candidate) = find_matching_binary(executable_dir, base_name) {
        return Ok(candidate);
    }

    if let Ok(paths) = env::var("PATH") {
        for dir in env::split_paths(&paths) {
            if let Some(candidate) = find_matching_binary(&dir, base_name) {
                return Ok(candidate);
            }
        }
    }

    Err(format!("未找到原生依赖二进制: {base_name}"))
}

fn find_matching_binary(dir: &Path, base_name: &str) -> Option<PathBuf> {
    let exact = if cfg!(windows) {
        dir.join(format!("{base_name}.exe"))
    } else {
        dir.join(base_name)
    };
    if exact.is_file() {
        return Some(exact);
    }

    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = path.file_name()?.to_str()?;
        if cfg!(windows) {
            if file_name == format!("{base_name}.exe")
                || file_name.starts_with(&format!("{base_name}-"))
            {
                return Some(path);
            }
        } else if file_name == base_name || file_name.starts_with(&format!("{base_name}-")) {
            return Some(path);
        }
    }
    None
}

fn encode_lines(
    spm_encode: &Path,
    spm_model_path: &Path,
    lines: &[String],
) -> Result<Vec<String>, String> {
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let output = run_process_capture(
        spm_encode,
        vec![
            "--model".to_string(),
            spm_model_path.display().to_string(),
            "--output_format".to_string(),
            "piece".to_string(),
        ],
        Some(lines.join("\n")),
    )?;
    Ok(output.lines().map(str::to_string).collect())
}

fn decode_lines(
    spm_decode: &Path,
    spm_model_path: &Path,
    lines: &[String],
) -> Result<Vec<String>, String> {
    let mut decoded = vec![String::new(); lines.len()];
    let active = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.trim().is_empty() {
                decoded[index] = String::new();
                None
            } else {
                Some((index, line.clone()))
            }
        })
        .collect::<Vec<_>>();
    if active.is_empty() {
        return Ok(decoded);
    }

    let output = run_process_capture(
        spm_decode,
        vec![
            "--model".to_string(),
            spm_model_path.display().to_string(),
            "--input_format".to_string(),
            "piece".to_string(),
        ],
        Some(
            active
                .iter()
                .map(|(_, line)| line.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )?;
    let output_lines = output.lines().collect::<Vec<_>>();
    if output_lines.len() != active.len() {
        return Err("spm_decode 输出行数与输入不一致".to_string());
    }

    for ((index, _), value) in active.into_iter().zip(output_lines) {
        decoded[index] = value.to_string();
    }
    Ok(decoded)
}

fn run_ct2_translator(
    ct2_translator: &Path,
    model_dir: &Path,
    source_lines: &[String],
    target_prefix_lines: &[String],
) -> Result<Vec<String>, String> {
    let temp_dir = TempDirGuard::new("muyu-translator")?;
    let source_path = temp_dir.path.join("source.txt");
    let target_path = temp_dir.path.join("target.txt");
    let output_path = temp_dir.path.join("output.txt");
    fs::write(&source_path, source_lines.join("\n"))
        .map_err(|error| format!("写入翻译源文件失败: {error}"))?;
    fs::write(&target_path, target_prefix_lines.join("\n"))
        .map_err(|error| format!("写入翻译前缀文件失败: {error}"))?;

    let intra_threads = std::thread::available_parallelism()
        .map(|value| value.get().min(8))
        .unwrap_or(4)
        .max(1);
    let args = vec![
        "--model".to_string(),
        model_dir.display().to_string(),
        "--src".to_string(),
        source_path.display().to_string(),
        "--tgt".to_string(),
        target_path.display().to_string(),
        "--out".to_string(),
        output_path.display().to_string(),
        "--device".to_string(),
        "cpu".to_string(),
        "--compute_type".to_string(),
        "auto".to_string(),
        "--inter_threads".to_string(),
        "1".to_string(),
        "--intra_threads".to_string(),
        intra_threads.to_string(),
        "--batch_size".to_string(),
        "32".to_string(),
        "--beam_size".to_string(),
        "4".to_string(),
        "--max_input_length".to_string(),
        "1024".to_string(),
        "--max_decoding_length".to_string(),
        "256".to_string(),
    ];
    let output = Command::new(ct2_translator)
        .args(&args)
        .output()
        .map_err(|error| format!("启动 ct2-translator 失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            "ct2-translator 执行失败".to_string()
        } else {
            format!("ct2-translator 执行失败: {detail}")
        });
    }

    let content = fs::read_to_string(&output_path)
        .map_err(|error| format!("读取 ct2-translator 输出失败: {error}"))?;
    Ok(content.lines().map(str::to_string).collect())
}

fn strip_target_prefix_and_eos(line: &str, target_lang_token: &str) -> String {
    let mut tokens = line
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if tokens.first().map(|token| token.as_str()) == Some(target_lang_token) {
        tokens.remove(0);
    }
    if tokens.last().map(|token| token.as_str()) == Some("</s>") {
        tokens.pop();
    }
    tokens.join(" ")
}

fn run_process_capture(
    program: &Path,
    args: Vec<String>,
    input: Option<String>,
) -> Result<String, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 {} 失败: {error}", program.display()))?;

    if let Some(payload) = input {
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(payload.as_bytes())
                .map_err(|error| format!("写入 {} 输入失败: {error}", program.display()))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待 {} 退出失败: {error}", program.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            format!("{} 执行失败", program.display())
        } else {
            format!("{} 执行失败: {detail}", program.display())
        });
    }

    String::from_utf8(output.stdout)
        .map_err(|error| format!("解析 {} 输出失败: {error}", program.display()))
}
