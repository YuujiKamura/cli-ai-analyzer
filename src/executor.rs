//! Gemini CLI executor

use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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

impl OutputFormat {
    fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
        }
    }
}

/// Request parameters for Gemini CLI
#[derive(Debug)]
pub struct GeminiRequest<'a> {
    /// The prompt to send
    pub prompt: &'a str,
    /// The model to use
    pub model: &'a str,
    /// Optional files to analyze
    pub files: Option<&'a [String]>,
    /// Output format
    pub output_format: OutputFormat,
}

impl<'a> GeminiRequest<'a> {
    /// Create a text request without files
    pub fn text(prompt: &'a str, model: &'a str) -> Self {
        Self {
            prompt,
            model,
            files: None,
            output_format: OutputFormat::Text,
        }
    }

    /// Create a text request with files
    pub fn text_with_files(prompt: &'a str, model: &'a str, files: &'a [String]) -> Self {
        Self {
            prompt,
            model,
            files: Some(files),
            output_format: OutputFormat::Text,
        }
    }

    /// Create a JSON request without files
    pub fn json(prompt: &'a str, model: &'a str) -> Self {
        Self {
            prompt,
            model,
            files: None,
            output_format: OutputFormat::Json,
        }
    }

    /// Create a JSON request with files
    pub fn json_with_files(prompt: &'a str, model: &'a str, files: &'a [String]) -> Self {
        Self {
            prompt,
            model,
            files: Some(files),
            output_format: OutputFormat::Json,
        }
    }
}

/// Get the Gemini CLI path
pub fn gemini_cmd_path(custom_path: Option<&str>) -> String {
    // Custom path takes priority
    if let Some(path) = custom_path {
        return path.to_string();
    }

    // Environment variable
    if let Ok(path) = std::env::var("GEMINI_CMD_PATH") {
        return path;
    }

    // OS default
    if cfg!(target_os = "windows") {
        "gemini.cmd".to_string()
    } else {
        "gemini".to_string()
    }
}

/// Run Gemini CLI with the given request
pub fn run_gemini(
    work_dir: &Path,
    request: &GeminiRequest<'_>,
    custom_gemini_path: Option<&str>,
) -> Result<String> {
    // Write prompt to file
    let prompt_file = work_dir.join("prompt.txt");
    fs::write(&prompt_file, request.prompt)?;

    let gemini_path = gemini_cmd_path(custom_gemini_path);

    // Build and execute script
    if cfg!(target_os = "windows") {
        run_gemini_windows(work_dir, &gemini_path, request)
    } else {
        run_gemini_unix(work_dir, &gemini_path, request)
    }
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
    let output = cmd.output()?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(clean_gemini_output(&result))
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
    let model = request.model;
    let output_format = request.output_format.as_str();

    if let Some(files) = request.files {
        let file_array = files
            .iter()
            .map(|f| format!("    '{}'", f.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
$files = @(
{}
)
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' -m {} -o {} $files
"#,
            file_array, gemini_path, model, output_format
        )
    } else {
        format!(
            r#"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8
Get-Content -Raw -Encoding UTF8 'prompt.txt' | & '{}' -m {} -o {}
"#,
            gemini_path, model, output_format
        )
    }
}

/// Build shell script for Unix
fn build_shell_script(gemini_path: &str, request: &GeminiRequest<'_>) -> String {
    let model = request.model;
    let output_format = request.output_format.as_str();

    if let Some(files) = request.files {
        let file_args = files
            .iter()
            .map(|f| format!("'{}'", f.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            r#"#!/bin/bash
cat prompt.txt | '{}' -m {} -o {} {}
"#,
            gemini_path, model, output_format, file_args
        )
    } else {
        format!(
            r#"#!/bin/bash
cat prompt.txt | '{}' -m {} -o {}
"#,
            gemini_path, model, output_format
        )
    }
}

/// Clean Gemini output by removing noise
pub fn clean_gemini_output(output: &str) -> String {
    output
        .lines()
        .filter(|line| {
            !line.contains("Loaded cached credentials")
                && !line.contains("Hook registry initialized")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Write error log for debugging
fn write_error_log(work_dir: &Path, detail: &str) {
    let log_path = work_dir.join("gemini-error.log");
    let _ = fs::write(log_path, detail);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_request_text() {
        let req = GeminiRequest::text("test prompt", "gemini-2.5-flash");
        assert_eq!(req.prompt, "test prompt");
        assert_eq!(req.model, "gemini-2.5-flash");
        assert!(req.files.is_none());
        assert_eq!(req.output_format, OutputFormat::Text);
    }

    #[test]
    fn test_gemini_request_json_with_files() {
        let files = vec!["a.pdf".to_string(), "b.pdf".to_string()];
        let req = GeminiRequest::json_with_files("test", "gemini-2.5-flash", &files);
        assert!(req.files.is_some());
        assert_eq!(req.output_format, OutputFormat::Json);
    }

    #[test]
    fn test_clean_gemini_output() {
        let output = "Loaded cached credentials\nActual content\nHook registry initialized\nMore content";
        let cleaned = clean_gemini_output(output);
        assert_eq!(cleaned, "Actual content\nMore content");
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
}
