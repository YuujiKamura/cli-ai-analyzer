//! # cli-ai-analyzer
//!
//! A universal AI analysis library supporting multiple AI CLI backends.
//!
//! ## Supported Backends
//!
//! - **Gemini** (default) - Google's Gemini CLI
//! - **Claude** - Anthropic's Claude CLI
//! - **Ollama** - Local LLM (planned)
//!
//! ## Features
//!
//! - File analysis (PDF, images, text)
//! - Text-only prompts
//! - Customizable models
//! - Backend switching
//! - Temporary directory management
//!
//! ## Example
//!
//! ```rust,no_run
//! use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};
//! use std::path::PathBuf;
//!
//! // Using Gemini (default)
//! let result = analyze(
//!     "Describe this document",
//!     &[PathBuf::from("document.pdf")],
//!     AnalyzeOptions::default(),
//! ).unwrap();
//!
//! // Using Claude
//! let result = analyze(
//!     "Describe this document",
//!     &[PathBuf::from("document.pdf")],
//!     AnalyzeOptions::default().with_backend(Backend::Claude),
//! ).unwrap();
//!
//! println!("{}", result);
//! ```

mod backend;
mod error;
mod executor;
mod temp;

pub use backend::Backend;
pub use error::{Error, Result};
pub use executor::{AiRequest, GeminiRequest, GeminiStats, OutputFormat, get_gemini_stats, check_gemini_status};

use std::path::Path;

/// Default Gemini model
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

/// Default Claude model
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";

/// Options for analysis
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Model to use
    pub model: String,
    /// Output format (text or json)
    pub output_format: OutputFormat,
    /// Custom CLI path (optional)
    pub cli_path: Option<String>,
    /// Backend to use (Gemini, Claude, etc.)
    pub backend: Backend,
    /// Partial JSON with null values to fill in
    pub partial_json: Option<String>,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            output_format: OutputFormat::Text,
            cli_path: None,
            backend: Backend::default(),
            partial_json: None,
        }
    }
}

impl AnalyzeOptions {
    /// Create options with a specific model
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Set the backend to use
    pub fn with_backend(mut self, backend: Backend) -> Self {
        // Update model to backend's default if using the old default
        if self.model == DEFAULT_MODEL && backend != Backend::Gemini {
            self.model = backend.default_model().to_string();
        }
        self.backend = backend;
        self
    }

    /// Set output format to JSON
    pub fn json(mut self) -> Self {
        self.output_format = OutputFormat::Json;
        self
    }

    /// Set custom CLI path
    pub fn with_cli_path(mut self, path: impl Into<String>) -> Self {
        self.cli_path = Some(path.into());
        self
    }

    /// Set custom Gemini CLI path (backward compatibility alias)
    pub fn with_gemini_path(mut self, path: impl Into<String>) -> Self {
        self.cli_path = Some(path.into());
        self
    }

    /// Deprecated: Use cli_path instead
    #[deprecated(since = "0.2.0", note = "Use cli_path field instead")]
    pub fn gemini_path(&self) -> Option<&String> {
        self.cli_path.as_ref()
    }

    /// Set partial JSON with null values to fill in
    pub fn with_partial_json(mut self, partial_json: impl Into<String>) -> Self {
        self.partial_json = Some(partial_json.into());
        self
    }
}

/// Analyze files with a prompt using Gemini CLI
///
/// # Arguments
///
/// * `prompt` - The prompt to send to Gemini
/// * `files` - Files to analyze (PDF, images, etc.)
/// * `options` - Analysis options
///
/// # Returns
///
/// The analysis result as a string
///
/// # Example
///
/// ```rust,no_run
/// use cli_ai_analyzer::{analyze, AnalyzeOptions};
/// use std::path::PathBuf;
///
/// let result = analyze(
///     "What is in this document?",
///     &[PathBuf::from("doc.pdf")],
///     AnalyzeOptions::default(),
/// ).unwrap();
/// ```
pub fn analyze<P: AsRef<Path>>(
    prompt: &str,
    files: &[P],
    options: AnalyzeOptions,
) -> Result<String> {
    let temp_dir = temp::create_temp_dir("cli-ai-analyzer")?;
    let result = analyze_in_dir(&temp_dir, prompt, files, options);
    temp::cleanup_temp_dir(&temp_dir);
    result
}

/// Analyze with a prompt only (no files)
///
/// # Arguments
///
/// * `prompt` - The prompt to send to the AI backend
/// * `options` - Analysis options
///
/// # Returns
///
/// The response as a string
///
/// # Example
///
/// ```rust,no_run
/// use cli_ai_analyzer::{prompt, AnalyzeOptions, Backend};
///
/// // Using Gemini (default)
/// let result = prompt(
///     "Explain the construction document verification process in Japanese",
///     AnalyzeOptions::default(),
/// ).unwrap();
///
/// // Using Claude
/// let result = prompt(
///     "Explain the construction document verification process in Japanese",
///     AnalyzeOptions::default().with_backend(Backend::Claude),
/// ).unwrap();
/// ```
pub fn prompt(prompt: &str, options: AnalyzeOptions) -> Result<String> {
    let temp_dir = temp::create_temp_dir("cli-ai-analyzer")?;
    let result = prompt_in_dir(&temp_dir, prompt, options);
    temp::cleanup_temp_dir(&temp_dir);
    result
}

/// Analyze files in a specific directory (for advanced use)
pub fn analyze_in_dir<P: AsRef<Path>>(
    work_dir: &Path,
    prompt: &str,
    files: &[P],
    options: AnalyzeOptions,
) -> Result<String> {
    // Copy files to work directory
    let mut file_names = Vec::new();
    for file in files {
        let file_path = file.as_ref();
        let file_name = file_path
            .file_name()
            .ok_or_else(|| Error::InvalidPath(file_path.to_path_buf()))?
            .to_string_lossy()
            .to_string();

        let dest = work_dir.join(&file_name);
        std::fs::copy(file_path, &dest)?;
        file_names.push(file_name);
    }

    let partial_json_ref = options.partial_json.as_deref();
    let request = AiRequest {
        prompt,
        model: &options.model,
        files: Some(&file_names),
        output_format: options.output_format,
        backend: options.backend,
        partial_json: partial_json_ref,
    };

    executor::run_ai(work_dir, &request, options.cli_path.as_deref())
}

/// Run a prompt without files in a specific directory
pub fn prompt_in_dir(work_dir: &Path, prompt: &str, options: AnalyzeOptions) -> Result<String> {
    let partial_json_ref = options.partial_json.as_deref();
    let request = AiRequest {
        prompt,
        model: &options.model,
        files: None,
        output_format: options.output_format,
        backend: options.backend,
        partial_json: partial_json_ref,
    };

    executor::run_ai(work_dir, &request, options.cli_path.as_deref())
}

/// Builder for complex analysis requests
#[derive(Debug)]
pub struct AnalysisBuilder {
    prompt: String,
    files: Vec<std::path::PathBuf>,
    options: AnalyzeOptions,
}

impl AnalysisBuilder {
    /// Create a new analysis builder
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            files: Vec::new(),
            options: AnalyzeOptions::default(),
        }
    }

    /// Add a file to analyze
    pub fn file(mut self, path: impl AsRef<Path>) -> Self {
        self.files.push(path.as_ref().to_path_buf());
        self
    }

    /// Add multiple files to analyze
    pub fn files<P: AsRef<Path>>(mut self, paths: impl IntoIterator<Item = P>) -> Self {
        for path in paths {
            self.files.push(path.as_ref().to_path_buf());
        }
        self
    }

    /// Set the model to use
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.options.model = model.into();
        self
    }

    /// Set output format to JSON
    pub fn json(mut self) -> Self {
        self.options.output_format = OutputFormat::Json;
        self
    }

    /// Set the backend to use
    pub fn backend(mut self, backend: Backend) -> Self {
        self.options = self.options.with_backend(backend);
        self
    }

    /// Set custom CLI path
    pub fn cli_path(mut self, path: impl Into<String>) -> Self {
        self.options.cli_path = Some(path.into());
        self
    }

    /// Set custom Gemini CLI path (backward compatibility alias)
    pub fn gemini_path(mut self, path: impl Into<String>) -> Self {
        self.options.cli_path = Some(path.into());
        self
    }

    /// Set partial JSON with null values to fill in
    pub fn partial_json(mut self, partial_json: impl Into<String>) -> Self {
        self.options.partial_json = Some(partial_json.into());
        self
    }

    /// Execute the analysis
    pub fn run(self) -> Result<String> {
        if self.files.is_empty() {
            prompt(&self.prompt, self.options)
        } else {
            analyze(&self.prompt, &self.files, self.options)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_options_default() {
        let opts = AnalyzeOptions::default();
        assert_eq!(opts.model, DEFAULT_MODEL);
        assert!(matches!(opts.output_format, OutputFormat::Text));
        assert_eq!(opts.backend, Backend::Gemini);
    }

    #[test]
    fn test_analyze_options_builder() {
        let opts = AnalyzeOptions::with_model("gemini-2.5-flash-exp")
            .json()
            .with_cli_path("/custom/path");

        assert_eq!(opts.model, "gemini-2.5-flash-exp");
        assert!(matches!(opts.output_format, OutputFormat::Json));
        assert_eq!(opts.cli_path, Some("/custom/path".to_string()));
    }

    #[test]
    fn test_analyze_options_with_backend() {
        let opts = AnalyzeOptions::default().with_backend(Backend::Claude);
        assert_eq!(opts.backend, Backend::Claude);
        assert_eq!(opts.model, DEFAULT_CLAUDE_MODEL);
    }

    #[test]
    fn test_analyze_options_with_backend_custom_model() {
        let opts = AnalyzeOptions::with_model("custom-model").with_backend(Backend::Claude);
        assert_eq!(opts.backend, Backend::Claude);
        // Custom model is preserved even with backend change
        assert_eq!(opts.model, "custom-model");
    }

    #[test]
    fn test_analysis_builder() {
        let builder = AnalysisBuilder::new("test prompt")
            .file("file1.pdf")
            .file("file2.pdf")
            .model("gemini-2.5-flash-exp")
            .json();

        assert_eq!(builder.prompt, "test prompt");
        assert_eq!(builder.files.len(), 2);
        assert_eq!(builder.options.model, "gemini-2.5-flash-exp");
    }

    #[test]
    fn test_analysis_builder_with_backend() {
        let builder = AnalysisBuilder::new("test prompt")
            .backend(Backend::Claude);

        assert_eq!(builder.options.backend, Backend::Claude);
        assert_eq!(builder.options.model, DEFAULT_CLAUDE_MODEL);
    }
}
