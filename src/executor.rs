//! AI CLI executor (Gemini, Claude, Ollama)

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::backend::{Backend, UsageMode};
use crate::error::{Error, Result};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

/// Windows flag to hide console window
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Output format for Gemini CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Plain text output
    Text,
    /// JSON output
    Json,
}


/// Request parameters for AI CLI
#[derive(Debug)]
pub struct AiRequest<'a> {
    /// The prompt to send
    pub prompt: &'a str,
    /// The model to use
    pub model: &'a str,
    /// Optional files to analyze
    pub files: Option<&'a [String]>,
    /// Output format
    pub output_format: OutputFormat,
    /// Backend to use
    pub backend: Backend,
    /// Partial JSON with null values to fill in
    pub partial_json: Option<&'a str>,
    /// Usage mode (billing model)
    pub usage_mode: UsageMode,
}

/// Alias for backward compatibility
pub type GeminiRequest<'a> = AiRequest<'a>;

impl<'a> AiRequest<'a> {
    /// Create a text request without files (defaults to Gemini)
    pub fn text(prompt: &'a str, model: &'a str) -> Self {
        Self {
            prompt,
            model,
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        }
    }

    /// Create a text request with files (defaults to Gemini)
    pub fn text_with_files(prompt: &'a str, model: &'a str, files: &'a [String]) -> Self {
        Self {
            prompt,
            model,
            files: Some(files),
            output_format: OutputFormat::Text,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        }
    }

    /// Create a JSON request without files (defaults to Gemini)
    pub fn json(prompt: &'a str, model: &'a str) -> Self {
        Self {
            prompt,
            model,
            files: None,
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        }
    }

    /// Create a JSON request with files (defaults to Gemini)
    pub fn json_with_files(prompt: &'a str, model: &'a str, files: &'a [String]) -> Self {
        Self {
            prompt,
            model,
            files: Some(files),
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        }
    }

    /// Set the backend to use
    pub fn with_backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
        self
    }

    /// Set partial JSON to fill in null values
    pub fn with_partial_json(mut self, partial_json: &'a str) -> Self {
        self.partial_json = Some(partial_json);
        self
    }

    /// Set usage mode (billing model)
    pub fn with_usage_mode(mut self, usage_mode: UsageMode) -> Self {
        self.usage_mode = usage_mode;
        self
    }
}

/// Get the CLI path for the specified backend
pub fn cli_cmd_path(backend: Backend, custom_path: Option<&str>) -> String {
    // Custom path takes priority
    if let Some(path) = custom_path {
        return path.to_string();
    }

    // Environment variable based on backend
    let env_var = match backend {
        Backend::Gemini => "GEMINI_CMD_PATH",
        Backend::Claude => "CLAUDE_CMD_PATH",
        Backend::Codex => "CODEX_CMD_PATH",
        Backend::Ollama => "OLLAMA_CMD_PATH",
    };

    if let Ok(path) = std::env::var(env_var) {
        return path;
    }

    // OS default
    if cfg!(target_os = "windows") {
        backend.windows_command().to_string()
    } else {
        backend.command().to_string()
    }
}

/// Get the Gemini CLI path (backward compatibility)
#[allow(dead_code)]
pub fn gemini_cmd_path(custom_path: Option<&str>) -> String {
    cli_cmd_path(Backend::Gemini, custom_path)
}

/// Run AI CLI with the given request
pub fn run_ai(
    work_dir: &Path,
    request: &AiRequest<'_>,
    custom_cli_path: Option<&str>,
    profile_path: Option<&Path>,
) -> Result<String> {
    let start = Instant::now();
    let result = (|| {
        // PayPerUse mode: call Gemini REST API directly (Gemini backend only)
        if request.usage_mode == UsageMode::PayPerUse {
            if request.backend != Backend::Gemini {
                return Err(Error::GeminiError(
                    "PayPerUse mode is only supported for Gemini backend.".into(),
                ));
            }
            return run_gemini_rest_api(work_dir, request);
        }

        let cli_path = cli_cmd_path(request.backend, custom_cli_path);

        // Build and execute script based on backend
        match request.backend {
            Backend::Gemini => {
                // Write prompt to file for Gemini (uses stdin)
                let prompt_file = work_dir.join("prompt.txt");
                fs::write(&prompt_file, request.prompt)?;

                if cfg!(target_os = "windows") {
                    run_gemini_windows(work_dir, &cli_path, request)
                } else {
                    run_gemini_unix(work_dir, &cli_path, request)
                }
            }
            Backend::Claude => {
                // Claude uses -p flag for prompt
                if cfg!(target_os = "windows") {
                    run_claude_windows(work_dir, &cli_path, request)
                } else {
                    run_claude_unix(work_dir, &cli_path, request)
                }
            }
            Backend::Codex => {
                // Codex uses exec subcommand with prompt as argument
                if cfg!(target_os = "windows") {
                    run_codex_windows(work_dir, &cli_path, request)
                } else {
                    run_codex_unix(work_dir, &cli_path, request)
                }
            }
            Backend::Ollama => {
                Err(Error::GeminiError("Ollama backend is not yet implemented".to_string()))
            }
        }
    })();

    if let Some(path) = profile_path {
        let _ = write_profile(path, request, start.elapsed(), result.as_ref().err(), false);
    }

    result
}

/// Run Gemini REST API directly (PayPerUse mode)
fn run_gemini_rest_api(
    work_dir: &Path,
    request: &AiRequest<'_>,
) -> Result<String> {
    let api_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
        Error::GeminiError(
            "GEMINI_API_KEY environment variable is not set. \
             Required for PayPerUse mode."
                .into(),
        )
    })?;

    let model = if request.model.is_empty() {
        "gemini-3-flash-preview"
    } else {
        request.model
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    // Build prompt text with JSON suffix if needed
    let prompt_text = if let Some(partial) = request.partial_json {
        format!(
            "{} Fill in the null values in this JSON based on the image: {}",
            request.prompt, partial
        )
    } else if request.output_format == OutputFormat::Json {
        format!("{} Respond with ONLY the JSON object.", request.prompt)
    } else {
        request.prompt.to_string()
    };

    // Build parts array
    let mut parts = Vec::new();

    // Add text part
    parts.push(serde_json::json!({"text": prompt_text}));

    // Add image parts (base64 encoded)
    if let Some(files) = request.files {
        for file_path in files {
            // Resolve path: if relative, join with work_dir
            let path = if Path::new(file_path).is_absolute() {
                std::path::PathBuf::from(file_path)
            } else {
                work_dir.join(file_path)
            };

            if !path.exists() {
                return Err(Error::FileNotFound(path));
            }

            let data = fs::read(&path)?;
            let b64 = BASE64_STANDARD.encode(&data);

            let mime_type = mime_guess::from_path(&path)
                .first_raw()
                .unwrap_or("application/octet-stream");

            parts.push(serde_json::json!({
                "inline_data": {
                    "mime_type": mime_type,
                    "data": b64
                }
            }));
        }
    }

    let body = serde_json::json!({
        "contents": [{
            "parts": parts
        }]
    });

    // Debug log the request (without base64 data)
    let debug_path = work_dir.join("debug.log");
    let _ = fs::write(
        &debug_path,
        format!(
            "PayPerUse REST API\nURL: https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent\nParts count: {}\n",
            model,
            parts.len()
        ),
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| Error::GeminiError(format!("Failed to create HTTP client: {}", e)))?;
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| Error::GeminiError(format!("HTTP request failed: {}", e)))?;

    let status = response.status();
    let response_text = response
        .text()
        .map_err(|e| Error::GeminiError(format!("Failed to read response body: {}", e)))?;

    // Debug log the response
    let _ = fs::OpenOptions::new()
        .append(true)
        .open(&debug_path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(
                format!(
                    "Status: {}\nResponse: {}\n",
                    status,
                    &response_text[..response_text.len().min(2000)]
                )
                .as_bytes(),
            )
        });

    if !status.is_success() {
        let detail = format!("Gemini API error ({}): {}", status, response_text);
        write_error_log(work_dir, &detail);

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(Error::AuthenticationFailed);
        }
        return Err(Error::GeminiError(detail));
    }

    // Parse the JSON response
    let resp: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
        Error::GeminiError(format!(
            "Failed to parse API response: {}. Body: {}",
            e,
            &response_text[..response_text.len().min(500)]
        ))
    })?;

    // Extract text from candidates[0].content.parts[0].text
    let text = resp
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| {
            // Check for error or blocked response
            let error_msg = resp
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("No text in response");
            Error::GeminiError(format!(
                "Unexpected API response: {}. Full: {}",
                error_msg,
                &response_text[..response_text.len().min(500)]
            ))
        })?;

    Ok(clean_ai_output(text))
}

/// Run Gemini CLI with the given request (backward compatibility)
#[allow(dead_code)]
pub fn run_gemini(
    work_dir: &Path,
    request: &GeminiRequest<'_>,
    custom_gemini_path: Option<&str>,
) -> Result<String> {
    run_ai(work_dir, request, custom_gemini_path, None)
}

/// Run AI CLI with `--resume latest` to continue an existing Gemini session.
/// Non-Gemini backends fall back to `run_ai` (no session concept).
pub(crate) fn run_ai_resume(
    work_dir: &Path,
    request: &AiRequest<'_>,
    custom_cli_path: Option<&str>,
    profile_path: Option<&Path>,
) -> Result<String> {
    if request.backend != Backend::Gemini {
        return run_ai(work_dir, request, custom_cli_path, profile_path);
    }

    let start = Instant::now();
    let cli_path = cli_cmd_path(request.backend, custom_cli_path);

    // Write prompt to file
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, request.prompt)?;

    if cfg!(target_os = "windows") {
        let ps_script = build_ps_script_resume(&cli_path, request);
        let script_file = work_dir.join("run.ps1");
        fs::write(&script_file, &ps_script)?;

        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_file.to_string_lossy(),
        ])
        .current_dir(work_dir);

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let result = execute_command(cmd, work_dir);
        if let Some(path) = profile_path {
            let _ = write_profile(path, request, start.elapsed(), result.as_ref().err(), true);
        }
        result
    } else {
        let shell_script = build_shell_script_resume(&cli_path, request);
        let script_file = work_dir.join("run.sh");
        fs::write(&script_file, &shell_script)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_file)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_file, perms)?;
        }

        let mut cmd = Command::new("bash");
        cmd.arg(&script_file).current_dir(work_dir);

        let result = execute_command(cmd, work_dir);
        if let Some(path) = profile_path {
            let _ = write_profile(path, request, start.elapsed(), result.as_ref().err(), true);
        }
        result
    }
}

fn write_profile(
    profile_path: &Path,
    request: &AiRequest<'_>,
    duration: Duration,
    error: Option<&Error>,
    resumed: bool,
) -> Result<()> {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();

    let file_count = request.files.map(|f| f.len()).unwrap_or(0);
    let record = serde_json::json!({
        "ts_ms": ts_ms,
        "duration_ms": duration.as_millis(),
        "backend": format!("{:?}", request.backend),
        "model": request.model,
        "output_format": format!("{:?}", request.output_format),
        "usage_mode": format!("{:?}", request.usage_mode),
        "file_count": file_count,
        "resumed": resumed,
        "ok": error.is_none(),
        "error": error.map(|e| e.to_string()).unwrap_or_default(),
    });

    let line = format!("{}\n", record);
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile_path)?;
    use std::io::Write;
    f.write_all(line.as_bytes())?;
    Ok(())
}

/// Run Gemini CLI on Windows using PowerShell
fn run_gemini_windows(
    work_dir: &Path,
    gemini_path: &str,
    request: &GeminiRequest<'_>,
) -> Result<String> {
    let ps_script = build_ps_script(gemini_path, request);
    let script_file = work_dir.join("run.ps1");
    fs::write(&script_file, &ps_script)?;

    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        &script_file.to_string_lossy(),
    ])
    .current_dir(work_dir);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    execute_command(cmd, work_dir)
}

/// Run Gemini CLI on Unix systems
fn run_gemini_unix(
    work_dir: &Path,
    gemini_path: &str,
    request: &GeminiRequest<'_>,
) -> Result<String> {
    let shell_script = build_shell_script(gemini_path, request);
    let script_file = work_dir.join("run.sh");
    fs::write(&script_file, &shell_script)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_file, perms)?;
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script_file).current_dir(work_dir);

    execute_command(cmd, work_dir)
}

/// Default timeout for CLI execution (180 seconds)
const CLI_TIMEOUT_SECS: u64 = 180;

/// Run a command with a timeout. Returns Output or Error on timeout/spawn failure.
fn run_with_timeout(cmd: &mut Command, timeout_secs: u64) -> Result<std::process::Output> {
    use std::time::{Duration, Instant};
    use std::io::Read;

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| Error::GeminiError(format!("spawn failed: {}", e)))?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished - collect output
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output { status, stdout, stderr });
            }
            Ok(None) => {
                // Still running - check timeout
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::GeminiError(format!(
                        "CLI timed out after {}s", timeout_secs
                    )));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                return Err(Error::GeminiError(format!("wait failed: {}", e)));
            }
        }
    }
}

/// Execute the command and process output (with timeout)
fn execute_command(mut cmd: Command, work_dir: &Path) -> Result<String> {
    // Debug: write command info
    let debug_path = work_dir.join("debug.log");
    let _ = fs::write(&debug_path, format!("Command: {:?}\nWorkDir: {:?}\n", cmd, work_dir));

    let output = run_with_timeout(&mut cmd, CLI_TIMEOUT_SECS)?;

    // Debug: append output info
    let debug_info = format!(
        "Status: {:?}\nStdout len: {}\nStderr len: {}\nStdout: {}\nStderr: {}\n",
        output.status,
        output.stdout.len(),
        output.stderr.len(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let _ = fs::OpenOptions::new()
        .append(true)
        .open(&debug_path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(debug_info.as_bytes())
        });

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).to_string();
        let cleaned = clean_ai_output(&result);

        // Gemini CLI sometimes exits 0 but writes errors to stderr with empty stdout
        if cleaned.is_empty() && !output.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let detail = format!("CLI returned empty output. Stderr: {}", stderr.trim());
            write_error_log(work_dir, &detail);
            return Err(Error::GeminiError(detail));
        }

        Ok(cleaned)
    } else {
        let status = output
            .status
            .code()
            .map(|c| format!("exit code {}", c))
            .unwrap_or_else(|| "terminated".to_string());
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        let detail = if stdout.trim().is_empty() {
            format!("{}: {}", status, stderr)
        } else {
            format!("{}: {}\n{}", status, stderr, stdout)
        };

        let detail = detail.trim().to_string();
        write_error_log(work_dir, &detail);

        // Check for specific error types
        if detail.contains("not found") || detail.contains("not recognized") {
            return Err(Error::GeminiCliNotFound);
        }
        if detail.contains("authenticate") || detail.contains("auth") {
            return Err(Error::AuthenticationFailed);
        }

        Err(Error::GeminiError(detail))
    }
}

/// Build PowerShell script for Windows
fn build_ps_script(gemini_path: &str, request: &GeminiRequest<'_>) -> String {
    let gemini_path = gemini_path.replace('\'', "''");
    // Always use text output for Gemini CLI - json output includes session metadata
    let output_format = "text";

    // Build model option - use default if model is empty
    let model = if request.model.is_empty() {
        "gemini-3-flash-preview"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    if let Some(files) = request.files {
        // Copy files to work dir with neutral names to prevent filename hints leaking to AI.
        // E.g. "isuzu_white_light_load.jpg" → "image_0.jpg"
        let mut copy_cmds = Vec::new();
        let mut neutral_refs = Vec::new();
        for (i, f) in files.iter().enumerate() {
            let ext = Path::new(f).extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg");
            let neutral = format!("image_{}.{}", i, ext);
            copy_cmds.push(format!("Copy-Item '{}' '{}'", f.replace('\'', "''"), neutral));
            neutral_refs.push(format!("@{}", neutral));
        }
        let file_refs = neutral_refs.join(" ");
        let copy_section = copy_cmds.join("\n");

        // Build JSON suffix based on partial_json or output_format
        let json_suffix = if let Some(partial) = request.partial_json {
            format!(" Fill in the null values in this JSON based on the image: {}", partial.replace('"', "`\""))
        } else if request.output_format == OutputFormat::Json {
            " Respond with ONLY the JSON object.".to_string()
        } else {
            String::new()
        };

        // Combine @file refs + prompt into stdin (no -p flag).
        // Gemini CLI auto-detects headless mode when stdin is piped.
        // Using -p with stdin triggers "Cannot use both positional and --prompt" error.
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
{copy_section}
$prompt = Get-Content -Raw -Encoding UTF8 'prompt.txt'
("{file_refs} " + $prompt + "{suffix}") | & '{gemini}' {model}-o {fmt}
"#,
            copy_section = copy_section,
            file_refs = file_refs,
            gemini = gemini_path,
            model = model_opt,
            fmt = output_format,
            suffix = json_suffix,
        )
    } else {
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' {}-o {}
"#,
            gemini_path, model_opt, output_format
        )
    }
}

/// Build shell script for Unix
fn build_shell_script(gemini_path: &str, request: &GeminiRequest<'_>) -> String {
    // Always use text output for Gemini CLI - json output includes session metadata
    let output_format = "text";

    // Build model option - use default if model is empty
    let model = if request.model.is_empty() {
        "gemini-3-flash-preview"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    if let Some(files) = request.files {
        // Copy files with neutral names to prevent filename hints leaking to AI
        let mut copy_cmds = Vec::new();
        let mut neutral_refs = Vec::new();
        for (i, f) in files.iter().enumerate() {
            let ext = Path::new(f).extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg");
            let neutral = format!("image_{}.{}", i, ext);
            copy_cmds.push(format!("cp '{}' '{}'", f.replace('\'', "'\\''"), neutral));
            neutral_refs.push(format!("@{}", neutral));
        }
        let file_refs = neutral_refs.join(" ");
        let copy_section = copy_cmds.join("\n");

        // Build JSON suffix based on partial_json or output_format
        let json_suffix = if let Some(partial) = request.partial_json {
            format!(" Fill in the null values in this JSON based on the image: {}", partial.replace('"', "\\\""))
        } else if request.output_format == OutputFormat::Json {
            " Respond with ONLY the JSON object.".to_string()
        } else {
            String::new()
        };

        // Combine @file refs + prompt into stdin (no -p flag).
        // Gemini CLI auto-detects headless mode when stdin is piped.
        format!(
            r#"#!/bin/bash
{copy_section}
prompt=$(cat prompt.txt)
echo "{file_refs} $prompt{suffix}" | '{gemini}' {model}-o {fmt}
"#,
            copy_section = copy_section,
            file_refs = file_refs,
            gemini = gemini_path,
            model = model_opt,
            fmt = output_format,
            suffix = json_suffix,
        )
    } else {
        format!(
            r#"#!/bin/bash
cat prompt.txt | '{}' {}-o {}
"#,
            gemini_path, model_opt, output_format
        )
    }
}

/// Build PowerShell script for Gemini resume (--resume latest, no file refs)
fn build_ps_script_resume(gemini_path: &str, request: &GeminiRequest<'_>) -> String {
    let gemini_path = gemini_path.replace('\'', "''");
    let output_format = "text";

    let model = if request.model.is_empty() {
        "gemini-3-flash-preview"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    let json_suffix = if let Some(partial) = request.partial_json {
        format!(" Fill in the null values in this JSON based on the image: {}", partial.replace('"', "`\""))
    } else if request.output_format == OutputFormat::Json {
        " Respond with ONLY the JSON object.".to_string()
    } else {
        String::new()
    };

    format!(
        r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
$prompt = Get-Content -Raw -Encoding UTF8 'prompt.txt'
($prompt + "{suffix}") | & '{gemini}' {model}--resume latest -o {fmt}
"#,
        gemini = gemini_path,
        model = model_opt,
        fmt = output_format,
        suffix = json_suffix,
    )
}

/// Build shell script for Gemini resume (--resume latest, no file refs)
fn build_shell_script_resume(gemini_path: &str, request: &GeminiRequest<'_>) -> String {
    let output_format = "text";

    let model = if request.model.is_empty() {
        "gemini-3-flash-preview"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    let json_suffix = if let Some(partial) = request.partial_json {
        format!(" Fill in the null values in this JSON based on the image: {}", partial.replace('"', "\\\""))
    } else if request.output_format == OutputFormat::Json {
        " Respond with ONLY the JSON object.".to_string()
    } else {
        String::new()
    };

    format!(
        r#"#!/bin/bash
prompt=$(cat prompt.txt)
echo "$prompt{suffix}" | '{gemini}' {model}--resume latest -o {fmt}
"#,
        gemini = gemini_path,
        model = model_opt,
        fmt = output_format,
        suffix = json_suffix,
    )
}

/// Run Claude CLI on Windows using PowerShell
/// Uses stdin to pass prompt to avoid encoding issues with non-ASCII characters
fn run_claude_windows(
    work_dir: &Path,
    claude_path: &str,
    request: &AiRequest<'_>,
) -> Result<String> {
    // Build prompt with file contents if files are provided
    let full_prompt = build_claude_prompt(request)?;

    // Write prompt to file for stdin input (avoids PowerShell encoding issues)
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, &full_prompt)?;

    let ps_script = build_claude_ps_script_stdin(claude_path, request);
    let script_file = work_dir.join("run.ps1");
    fs::write(&script_file, &ps_script)?;

    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        &script_file.to_string_lossy(),
    ])
    .current_dir(work_dir);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    execute_command(cmd, work_dir)
}

/// Run Claude CLI on Unix systems
/// Uses stdin to pass prompt to avoid encoding issues with non-ASCII characters
fn run_claude_unix(
    work_dir: &Path,
    claude_path: &str,
    request: &AiRequest<'_>,
) -> Result<String> {
    // Build prompt with file contents if files are provided
    let full_prompt = build_claude_prompt(request)?;

    // Write prompt to file for stdin input
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, &full_prompt)?;

    let shell_script = build_claude_shell_script_stdin(claude_path, request);
    let script_file = work_dir.join("run.sh");
    fs::write(&script_file, &shell_script)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_file, perms)?;
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script_file).current_dir(work_dir);

    execute_command(cmd, work_dir)
}

/// Build PowerShell script for Claude on Windows (legacy, uses quoted prompt)
/// Claude CLI format: claude -p "prompt" --model model [--output-format format] [--file file1 file2...]
#[allow(dead_code)]
fn build_claude_ps_script(claude_path: &str, request: &AiRequest<'_>) -> String {
    let claude_path = claude_path.replace('\'', "''");
    let model = request.model;
    let prompt = request.prompt.replace('\'', "''").replace('`', "``");

    // Build base command
    let mut script = format!(
        r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Remove-Item Env:\CLAUDECODE -ErrorAction SilentlyContinue
"#
    );

    // Build the command with arguments
    if let Some(files) = request.files {
        let file_array = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");

        script.push_str(&format!(
            r#"$files = @({})
& '{}' -p '{}' --model {} --output-format text --file $files
"#,
            file_array, claude_path, prompt, model
        ));
    } else {
        script.push_str(&format!(
            r#"& '{}' -p '{}' --model {} --output-format text
"#,
            claude_path, prompt, model
        ));
    }

    script
}

/// Build shell script for Claude on Unix (legacy, uses quoted prompt)
/// Claude CLI format: claude -p "prompt" --model model [--output-format format] [--file file1 file2...]
#[allow(dead_code)]
fn build_claude_shell_script(claude_path: &str, request: &AiRequest<'_>) -> String {
    let model = request.model;
    let prompt = request.prompt.replace('\'', "'\\''");

    if let Some(files) = request.files {
        let file_args = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            r#"#!/bin/bash
unset CLAUDECODE
'{}' -p '{}' --model {} --output-format text --file {}
"#,
            claude_path, prompt, model, file_args
        )
    } else {
        format!(
            r#"#!/bin/bash
unset CLAUDECODE
'{}' -p '{}' --model {} --output-format text
"#,
            claude_path, prompt, model
        )
    }
}

/// Build the full prompt for Claude, including file contents if provided
fn build_claude_prompt(request: &AiRequest<'_>) -> Result<String> {
    if let Some(files) = request.files {
        let mut full_prompt = String::new();

        // Add file contents
        for file_path in files {
            let path = Path::new(file_path);
            if path.exists() {
                let content = fs::read_to_string(path).map_err(|e| {
                    Error::GeminiError(format!("Failed to read file {}: {}", file_path, e))
                })?;
                full_prompt.push_str(&format!(
                    "=== File: {} ===\n{}\n\n",
                    file_path, content
                ));
            }
        }

        // Add the original prompt
        full_prompt.push_str(request.prompt);
        Ok(full_prompt)
    } else {
        Ok(request.prompt.to_string())
    }
}

/// Build PowerShell script for Claude on Windows using stdin
/// Claude CLI: cat prompt.txt | claude -p - --model model --output-format text [--file file1 file2...]
fn build_claude_ps_script_stdin(claude_path: &str, request: &AiRequest<'_>) -> String {
    let claude_path = claude_path.replace('\'', "''");
    let model = request.model;

    if let Some(files) = request.files {
        let file_array = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Remove-Item Env:\CLAUDECODE -ErrorAction SilentlyContinue
$files = @({})
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' -p - --model {} --output-format text --file $files
"#,
            file_array, claude_path, model
        )
    } else {
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Remove-Item Env:\CLAUDECODE -ErrorAction SilentlyContinue
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' -p - --model {} --output-format text
"#,
            claude_path, model
        )
    }
}

/// Build shell script for Claude on Unix using stdin
/// Claude CLI: cat prompt.txt | claude -p - --model model --output-format text [--file file1 file2...]
fn build_claude_shell_script_stdin(claude_path: &str, request: &AiRequest<'_>) -> String {
    let model = request.model;

    if let Some(files) = request.files {
        let file_args = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            r#"#!/bin/bash
unset CLAUDECODE
cat prompt.txt | '{}' -p - --model {} --output-format text --file {}
"#,
            claude_path, model, file_args
        )
    } else {
        format!(
            r#"#!/bin/bash
unset CLAUDECODE
cat prompt.txt | '{}' -p - --model {} --output-format text
"#,
            claude_path, model
        )
    }
}

/// Run Codex CLI on Windows using PowerShell
/// Uses stdin to pass prompt to avoid encoding issues
fn run_codex_windows(
    work_dir: &Path,
    codex_path: &str,
    request: &AiRequest<'_>,
) -> Result<String> {
    // Build prompt with file contents if files are provided
    let full_prompt = build_codex_prompt(request)?;

    // Write prompt to file for stdin input
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, &full_prompt)?;

    let ps_script = build_codex_ps_script(codex_path, request);
    let script_file = work_dir.join("run.ps1");
    fs::write(&script_file, &ps_script)?;

    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        &script_file.to_string_lossy(),
    ])
    .current_dir(work_dir);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    execute_command(cmd, work_dir)
}

/// Run Codex CLI on Unix systems
/// Uses stdin to pass prompt to avoid encoding issues
fn run_codex_unix(
    work_dir: &Path,
    codex_path: &str,
    request: &AiRequest<'_>,
) -> Result<String> {
    // Build prompt with file contents if files are provided
    let full_prompt = build_codex_prompt(request)?;

    // Write prompt to file for stdin input
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, &full_prompt)?;

    let shell_script = build_codex_shell_script(codex_path, request);
    let script_file = work_dir.join("run.sh");
    fs::write(&script_file, &shell_script)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_file)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_file, perms)?;
    }

    let mut cmd = Command::new("bash");
    cmd.arg(&script_file).current_dir(work_dir);

    execute_command(cmd, work_dir)
}

/// Build the full prompt for Codex, including file contents if provided
fn build_codex_prompt(request: &AiRequest<'_>) -> Result<String> {
    if let Some(files) = request.files {
        let mut full_prompt = String::new();

        // Add file contents
        for file_path in files {
            let path = Path::new(file_path);
            if path.exists() {
                let content = fs::read_to_string(path).map_err(|e| {
                    Error::GeminiError(format!("Failed to read file {}: {}", file_path, e))
                })?;
                full_prompt.push_str(&format!(
                    "=== File: {} ===\n{}\n\n",
                    file_path, content
                ));
            }
        }

        // Add the original prompt
        full_prompt.push_str(request.prompt);
        Ok(full_prompt)
    } else {
        Ok(request.prompt.to_string())
    }
}

/// Build PowerShell script for Codex on Windows
/// Codex CLI format: echo "prompt" | codex exec -m model -
/// Uses stdin to avoid encoding issues with quoted prompts
fn build_codex_ps_script(codex_path: &str, request: &AiRequest<'_>) -> String {
    let codex_path = codex_path.replace('\'', "''");
    let model = request.model;

    format!(
        r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' exec --skip-git-repo-check -m {} -
"#,
        codex_path, model
    )
}

/// Build shell script for Codex on Unix
/// Codex CLI format: cat prompt.txt | codex exec -m model -
/// Uses stdin to avoid encoding issues with quoted prompts
fn build_codex_shell_script(codex_path: &str, request: &AiRequest<'_>) -> String {
    let model = request.model;

    format!(
        r#"#!/bin/bash
cat prompt.txt | '{}' exec --skip-git-repo-check -m {} -
"#,
        codex_path, model
    )
}

/// Clean AI output by removing noise
pub fn clean_ai_output(output: &str) -> String {
    let filtered: String = output
        .lines()
        .filter(|line| {
            // Gemini noise
            !line.contains("Loaded cached credentials")
                && !line.contains("Hook registry initialized")
                && !line.contains("YOLO mode is enabled")
                // Claude noise (if any)
                && !line.starts_with("Loading")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Strip markdown code blocks if present (e.g., ```json...```)
    strip_markdown_code_blocks(&filtered)
}

/// Strip markdown code block markers from output
fn strip_markdown_code_blocks(s: &str) -> String {
    let trimmed = s.trim();

    // Check for ```json or ``` at start
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }

    if let Some(rest) = trimmed.strip_prefix("```") {
        // Could be ```\n or just ```
        let rest = rest.trim_start_matches(|c: char| c.is_alphabetic()); // strip language hint
        if let Some(content) = rest.strip_suffix("```") {
            return content.trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Alias for backward compatibility
#[allow(dead_code)]
pub fn clean_gemini_output(output: &str) -> String {
    clean_ai_output(output)
}

/// Write error log for debugging
fn write_error_log(work_dir: &Path, detail: &str) {
    let log_path = work_dir.join("gemini-error.log");
    let _ = fs::write(log_path, detail);
}

/// Gemini usage stats and rate limit info
#[derive(Debug, Clone, Default)]
pub struct GeminiStats {
    /// Whether the API is accessible
    pub is_available: bool,
    /// Error message if rate limited
    pub rate_limit_message: Option<String>,
    /// Requests per minute limit (from error)
    pub rpm_limit: Option<u32>,
    /// Tokens per minute limit (from error)
    pub tpm_limit: Option<u64>,
    /// Retry after seconds (from error)
    pub retry_after_seconds: Option<u32>,
    /// Raw response for debugging
    pub raw_response: String,
}

impl GeminiStats {
    /// Parse stats from error output
    pub fn from_error(error_msg: &str) -> Self {
        let mut stats = Self {
            is_available: false,
            raw_response: error_msg.to_string(),
            ..Default::default()
        };

        // Parse rate limit info from error messages
        // Example: "Resource has been exhausted (e.g. check quota)."
        // Example: "429 Too Many Requests"
        // Example: "RATE_LIMIT_EXCEEDED"
        let msg_lower = error_msg.to_lowercase();

        if msg_lower.contains("rate") || msg_lower.contains("quota") || msg_lower.contains("429") {
            stats.rate_limit_message = Some(error_msg.to_string());

            // Try to extract retry time
            if let Some(seconds) = extract_number_after(error_msg, "retry") {
                stats.retry_after_seconds = Some(seconds as u32);
            }
            if let Some(seconds) = extract_number_after(error_msg, "wait") {
                stats.retry_after_seconds = Some(seconds as u32);
            }
        }

        stats
    }

    /// Create stats for successful response
    pub fn available() -> Self {
        Self {
            is_available: true,
            ..Default::default()
        }
    }
}

/// Extract number appearing after a keyword
fn extract_number_after(s: &str, keyword: &str) -> Option<u64> {
    let lower = s.to_lowercase();
    if let Some(pos) = lower.find(keyword) {
        let after = &s[pos..];
        after
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()
    } else {
        None
    }
}

/// Check Gemini API availability with a minimal request
pub fn check_gemini_status(custom_gemini_path: Option<&str>) -> Result<GeminiStats> {
    let gemini_path = gemini_cmd_path(custom_gemini_path);
    let work_dir = std::env::temp_dir();

    // Send a minimal prompt to check if API is accessible
    let test_prompt = "respond with just: ok";

    #[cfg(target_os = "windows")]
    let result = {
        let ps_script = format!(
            r#"& '{}' -p '{}'"#,
            gemini_path.replace('\'', "''"),
            test_prompt.replace('\'', "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &ps_script]);
        cmd.creation_flags(CREATE_NO_WINDOW);

        execute_command(cmd, &work_dir)
    };

    #[cfg(not(target_os = "windows"))]
    let result = {
        let mut cmd = Command::new(&gemini_path);
        cmd.args(["-p", test_prompt]);

        execute_command(cmd, &work_dir)
    };

    match result {
        Ok(output) => {
            let cleaned = clean_ai_output(&output);
            if cleaned.to_lowercase().contains("ok") {
                Ok(GeminiStats::available())
            } else {
                // Might be rate limited with a specific message
                Ok(GeminiStats::from_error(&output))
            }
        }
        Err(e) => {
            Ok(GeminiStats::from_error(&e.to_string()))
        }
    }
}

/// Alias for backward compatibility
pub fn get_gemini_stats(custom_gemini_path: Option<&str>) -> Result<GeminiStats> {
    check_gemini_status(custom_gemini_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_request_text() {
        let req = AiRequest::text("test prompt", "gemini-3-flash-preview");
        assert_eq!(req.prompt, "test prompt");
        assert_eq!(req.model, "gemini-3-flash-preview");
        assert!(req.files.is_none());
        assert_eq!(req.output_format, OutputFormat::Text);
        assert_eq!(req.backend, Backend::Gemini);
    }

    #[test]
    fn test_gemini_request_json_with_files() {
        let files = vec!["a.pdf".to_string(), "b.pdf".to_string()];
        let req = AiRequest::json_with_files("test", "gemini-3-flash-preview", &files);
        assert!(req.files.is_some());
        assert_eq!(req.output_format, OutputFormat::Json);
    }

    #[test]
    fn test_ai_request_with_backend() {
        let req = AiRequest::text("test", "claude-sonnet-4-20250514")
            .with_backend(Backend::Claude);
        assert_eq!(req.backend, Backend::Claude);
    }

    #[test]
    fn test_clean_ai_output() {
        let output = "Loaded cached credentials\nActual content\nHook registry initialized\nMore content";
        let cleaned = clean_ai_output(output);
        assert_eq!(cleaned, "Actual content\nMore content");
    }

    #[test]
    fn test_clean_gemini_output_alias() {
        let output = "Loaded cached credentials\nActual content";
        let cleaned = clean_gemini_output(output);
        assert_eq!(cleaned, "Actual content");
    }

    #[test]
    fn test_cli_cmd_path_gemini_default() {
        let path = cli_cmd_path(Backend::Gemini, None);
        if cfg!(target_os = "windows") {
            assert_eq!(path, "gemini.cmd");
        } else {
            assert_eq!(path, "gemini");
        }
    }

    #[test]
    fn test_cli_cmd_path_claude_default() {
        let path = cli_cmd_path(Backend::Claude, None);
        if cfg!(target_os = "windows") {
            assert_eq!(path, "claude.cmd");
        } else {
            assert_eq!(path, "claude");
        }
    }

    #[test]
    fn test_cli_cmd_path_custom() {
        let path = cli_cmd_path(Backend::Gemini, Some("/custom/gemini"));
        assert_eq!(path, "/custom/gemini");
    }

    #[test]
    fn test_gemini_cmd_path_default() {
        let path = gemini_cmd_path(None);
        if cfg!(target_os = "windows") {
            assert_eq!(path, "gemini.cmd");
        } else {
            assert_eq!(path, "gemini");
        }
    }

    #[test]
    fn test_gemini_cmd_path_custom() {
        let path = gemini_cmd_path(Some("/custom/gemini"));
        assert_eq!(path, "/custom/gemini");
    }

    #[test]
    fn test_build_claude_shell_script_no_files() {
        let req = AiRequest {
            prompt: "test prompt",
            model: "claude-sonnet-4-20250514",
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Claude,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_claude_shell_script("claude", &req);
        assert!(script.contains("claude"));
        assert!(script.contains("-p 'test prompt'"));
        assert!(script.contains("--model claude-sonnet-4-20250514"));
        assert!(script.contains("--output-format text"));
        assert!(!script.contains("--file"));
    }

    #[test]
    fn test_build_claude_shell_script_with_files() {
        let files = vec!["doc.pdf".to_string(), "image.png".to_string()];
        let req = AiRequest {
            prompt: "analyze these",
            model: "claude-sonnet-4-20250514",
            files: Some(&files),
            output_format: OutputFormat::Text,
            backend: Backend::Claude,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_claude_shell_script("claude", &req);
        assert!(script.contains("--file"));
        assert!(script.contains("'doc.pdf'"));
        assert!(script.contains("'image.png'"));
    }

    #[test]
    fn test_cli_cmd_path_codex_default() {
        let path = cli_cmd_path(Backend::Codex, None);
        if cfg!(target_os = "windows") {
            assert_eq!(path, "codex.cmd");
        } else {
            assert_eq!(path, "codex");
        }
    }

    #[test]
    fn test_build_codex_shell_script_stdin() {
        let req = AiRequest {
            prompt: "test prompt",
            model: "gpt-4.1",
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Codex,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_codex_shell_script("codex", &req);
        assert!(script.contains("codex"));
        assert!(script.contains("exec"));
        assert!(script.contains("-m gpt-4.1"));
        // Now uses stdin with '-' instead of quoted prompt
        assert!(script.contains("cat prompt.txt"));
        assert!(script.contains(" -"));
    }

    #[test]
    fn test_build_codex_prompt_no_files() {
        let req = AiRequest {
            prompt: "test prompt",
            model: "gpt-4.1",
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Codex,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let prompt = build_codex_prompt(&req).unwrap();
        assert_eq!(prompt, "test prompt");
    }

    #[test]
    fn test_build_codex_ps_script_stdin() {
        let req = AiRequest {
            prompt: "test prompt",
            model: "gpt-4.1",
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Codex,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_codex_ps_script("codex", &req);
        assert!(script.contains("codex"));
        assert!(script.contains("exec"));
        assert!(script.contains("-m gpt-4.1"));
        // Now uses stdin with '-' instead of quoted prompt
        assert!(script.contains("Get-Content -Raw -Encoding UTF8 'prompt.txt'"));
        assert!(script.contains(" -"));
    }

    #[test]
    fn test_clean_ai_output_empty_triggers_stderr_path() {
        // When AI output is empty after cleaning, execute_command checks stderr
        assert_eq!(clean_ai_output(""), "");
        assert_eq!(clean_ai_output("   "), "");
        assert_eq!(clean_ai_output("Loaded cached credentials\nHook registry initialized"), "");
        // Non-empty output would not trigger stderr check
        assert!(!clean_ai_output("actual response").is_empty());
    }

    // === Regression tests: Gemini CLI v0.27.0 rejects "-p" when stdin is piped ===
    // The old build_ps_script() put @file.jpg references inside a -p flag argument,
    // which caused "Cannot use both positional and --prompt" errors. The fix was to
    // pipe everything through stdin without -p. These tests guard against that regression.

    #[test]
    fn test_build_ps_script_with_files_uses_stdin_not_p() {
        let files = vec!["C:\\path\\to\\descriptive_name.jpg".to_string()];
        let req = AiRequest {
            prompt: "test prompt",
            model: "gemini-2.5-pro",
            files: Some(&files),
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_ps_script("gemini.cmd", &req);

        // Must copy file with neutral name to prevent filename hints leaking to AI
        assert!(
            script.contains("Copy-Item") && script.contains("image_0.jpg"),
            "PS script must copy files with neutral names, got: {}",
            script
        );

        // Must use neutral @filename, NOT the original descriptive name
        assert!(
            script.contains("@image_0.jpg"),
            "PS script must use neutral @filename reference, got: {}",
            script
        );
        assert!(
            !script.contains("@C:\\") && !script.contains("@descriptive_name"),
            "PS script must NOT leak original filename in @ reference, got: {}",
            script
        );

        // Must NOT contain -p flag (Gemini CLI v0.27.0 rejects -p with piped stdin)
        assert!(
            !script.contains(" -p "),
            "PS script with files must NOT use -p flag (breaks Gemini CLI v0.27.0), got: {}",
            script
        );

        // Must pipe content through stdin using PowerShell pipe operator
        assert!(
            script.contains("| &"),
            "PS script with files must pipe content via stdin (| &), got: {}",
            script
        );
    }

    #[test]
    fn test_build_shell_script_with_files_uses_stdin_not_p() {
        let files = vec!["/path/to/descriptive_name.jpg".to_string()];
        let req = AiRequest {
            prompt: "test prompt",
            model: "gemini-2.5-pro",
            files: Some(&files),
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_shell_script("gemini", &req);

        // Must copy file with neutral name
        assert!(
            script.contains("cp ") && script.contains("image_0.jpg"),
            "Shell script must copy files with neutral names, got: {}",
            script
        );

        // Must use neutral @filename, NOT the original descriptive name
        assert!(
            script.contains("@image_0.jpg"),
            "Shell script must use neutral @filename reference, got: {}",
            script
        );
        assert!(
            !script.contains("@descriptive_name"),
            "Shell script must NOT leak original filename in @ reference, got: {}",
            script
        );

        // Must NOT contain -p flag (Gemini CLI v0.27.0 rejects -p with piped stdin)
        assert!(
            !script.contains(" -p "),
            "Shell script with files must NOT use -p flag (breaks Gemini CLI v0.27.0), got: {}",
            script
        );

        // Must pipe content through stdin using echo
        assert!(
            script.contains("echo"),
            "Shell script with files must pipe content via echo, got: {}",
            script
        );
    }

    #[test]
    fn test_build_ps_script_no_files_uses_stdin() {
        let req = AiRequest {
            prompt: "test prompt",
            model: "gemini-2.5-pro",
            files: None,
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_ps_script("gemini.cmd", &req);

        // No-files case still pipes prompt.txt through stdin
        assert!(
            script.contains("Get-Content"),
            "PS script without files must pipe prompt.txt via Get-Content, got: {}",
            script
        );
        assert!(
            script.contains("| &"),
            "PS script without files must pipe to gemini via | &, got: {}",
            script
        );

        // Must NOT contain -p flag even without files
        assert!(
            !script.contains(" -p "),
            "PS script without files must NOT use -p flag, got: {}",
            script
        );
    }

    // === Resume script tests ===

    #[test]
    fn test_build_ps_script_resume_no_file_refs() {
        let req = AiRequest {
            prompt: "estimate fill ratios",
            model: "gemini-3-flash-preview",
            files: None,
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_ps_script_resume("gemini.cmd", &req);

        // Must contain --resume latest
        assert!(
            script.contains("--resume latest"),
            "Resume PS script must contain --resume latest, got: {}",
            script
        );

        // Must NOT contain @image references
        assert!(
            !script.contains("@image"),
            "Resume PS script must NOT contain @image refs, got: {}",
            script
        );

        // Must NOT contain Copy-Item (no file copying)
        assert!(
            !script.contains("Copy-Item"),
            "Resume PS script must NOT contain Copy-Item, got: {}",
            script
        );

        // Must contain --yolo
        assert!(
            script.contains("--yolo"),
            "Resume PS script must contain --yolo, got: {}",
            script
        );

        // Must contain JSON suffix for Json output format
        assert!(
            script.contains("Respond with ONLY the JSON object"),
            "Resume PS script with Json format must contain JSON suffix, got: {}",
            script
        );
    }

    #[test]
    fn test_build_shell_script_resume_no_file_refs() {
        let req = AiRequest {
            prompt: "estimate fill ratios",
            model: "gemini-3-flash-preview",
            files: None,
            output_format: OutputFormat::Json,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_shell_script_resume("gemini", &req);

        // Must contain --resume latest
        assert!(
            script.contains("--resume latest"),
            "Resume shell script must contain --resume latest, got: {}",
            script
        );

        // Must NOT contain @image references
        assert!(
            !script.contains("@image"),
            "Resume shell script must NOT contain @image refs, got: {}",
            script
        );

        // Must NOT contain cp (no file copying)
        assert!(
            !script.contains("cp '"),
            "Resume shell script must NOT contain cp file copy, got: {}",
            script
        );

        // Must contain --yolo
        assert!(
            script.contains("--yolo"),
            "Resume shell script must contain --yolo, got: {}",
            script
        );

        // Must contain JSON suffix for Json output format
        assert!(
            script.contains("Respond with ONLY the JSON object"),
            "Resume shell script with Json format must contain JSON suffix, got: {}",
            script
        );
    }

    #[test]
    fn test_build_ps_script_resume_text_format_no_json_suffix() {
        let req = AiRequest {
            prompt: "describe the truck",
            model: "gemini-3-flash-preview",
            files: None,
            output_format: OutputFormat::Text,
            backend: Backend::Gemini,
            partial_json: None,
            usage_mode: UsageMode::TimeBasedQuota,
        };
        let script = build_ps_script_resume("gemini.cmd", &req);

        assert!(script.contains("--resume latest"));
        assert!(
            !script.contains("Respond with ONLY"),
            "Resume PS script with Text format must NOT contain JSON suffix, got: {}",
            script
        );
    }
}
