//! AI CLI executor (Gemini, Claude, Ollama)

use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::backend::Backend;
use crate::error::{Error, Result};

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
        }
    }

    /// Set the backend to use
    pub fn with_backend(mut self, backend: Backend) -> Self {
        self.backend = backend;
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
) -> Result<String> {
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
}

/// Run Gemini CLI with the given request (backward compatibility)
#[allow(dead_code)]
pub fn run_gemini(
    work_dir: &Path,
    request: &GeminiRequest<'_>,
    custom_gemini_path: Option<&str>,
) -> Result<String> {
    run_ai(work_dir, request, custom_gemini_path)
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

/// Execute the command and process output
fn execute_command(mut cmd: Command, work_dir: &Path) -> Result<String> {
    // Debug: write command info
    let debug_path = work_dir.join("debug.log");
    let _ = fs::write(&debug_path, format!("Command: {:?}\nWorkDir: {:?}\n", cmd, work_dir));

    let output = cmd.output()?;

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
        Ok(clean_ai_output(&result))
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
        "gemini-2.5-flash"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    if let Some(files) = request.files {
        // Use @filename syntax to reference files in gemini CLI
        // File reference first, then prompt (matches successful manual test)
        let file_refs = files
            .iter()
            .map(|f| format!("@{}", f.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(" ");

        // Add JSON reinforcement suffix if prompt requests JSON output
        let json_suffix = if request.output_format == OutputFormat::Json {
            " Respond with ONLY the JSON object."
        } else {
            ""
        };

        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
$prompt = Get-Content -Raw -Encoding UTF8 'prompt.txt'
$fullPrompt = "{} " + $prompt + "{}"
& '{}' {}--yolo -o {} -p $fullPrompt
"#,
            file_refs, json_suffix, gemini_path, model_opt, output_format
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
        "gemini-2.5-flash"
    } else {
        request.model
    };
    let model_opt = format!("-m {} ", model);

    if let Some(files) = request.files {
        // Use @filename syntax to reference files in gemini CLI
        let file_refs = files
            .iter()
            .map(|f| format!("@{}", f.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");

        // Add JSON reinforcement suffix if prompt requests JSON output
        let json_suffix = if request.output_format == OutputFormat::Json {
            " Respond with ONLY the JSON object."
        } else {
            ""
        };

        format!(
            r#"#!/bin/bash
prompt=$(cat prompt.txt)
'{}' {}--yolo -o {} -p "{} $prompt{}"
"#,
            gemini_path, model_opt, output_format, file_refs, json_suffix
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
'{}' -p '{}' --model {} --output-format text --file {}
"#,
            claude_path, prompt, model, file_args
        )
    } else {
        format!(
            r#"#!/bin/bash
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
$files = @({})
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' -p - --model {} --output-format text --file $files
"#,
            file_array, claude_path, model
        )
    } else {
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
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
cat prompt.txt | '{}' -p - --model {} --output-format text --file {}
"#,
            claude_path, model, file_args
        )
    } else {
        format!(
            r#"#!/bin/bash
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
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' exec -m {} -
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
cat prompt.txt | '{}' exec -m {} -
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
        let req = AiRequest::text("test prompt", "gemini-2.5-flash");
        assert_eq!(req.prompt, "test prompt");
        assert_eq!(req.model, "gemini-2.5-flash");
        assert!(req.files.is_none());
        assert_eq!(req.output_format, OutputFormat::Text);
        assert_eq!(req.backend, Backend::Gemini);
    }

    #[test]
    fn test_gemini_request_json_with_files() {
        let files = vec!["a.pdf".to_string(), "b.pdf".to_string()];
        let req = AiRequest::json_with_files("test", "gemini-2.5-flash", &files);
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
        };
        let script = build_codex_ps_script("codex", &req);
        assert!(script.contains("codex"));
        assert!(script.contains("exec"));
        assert!(script.contains("-m gpt-4.1"));
        // Now uses stdin with '-' instead of quoted prompt
        assert!(script.contains("Get-Content -Raw -Encoding UTF8 'prompt.txt'"));
        assert!(script.contains(" -"));
    }
}
