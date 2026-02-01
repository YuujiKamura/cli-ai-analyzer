//! # gemini-analyzer
//!
//! A universal AI analysis library powered by Gemini CLI.
//!
//! ## Features
//!
//! - File analysis (PDF, images, text)
//! - Text-only prompts
//! - Customizable models
//! - Temporary directory management
//!
//! ## Example
//!
//! ```rust,no_run
//! use gemini_analyzer::{analyze, AnalyzeOptions};
//! use std::path::PathBuf;
//!
//! let result = analyze(
//!     "Describe this document",
//!     &[PathBuf::from("document.pdf")],
//!     AnalyzeOptions::default(),
//! ).unwrap();
//!
//! println!("{}", result);
//! ```

mod error;
mod executor;
mod temp;

pub use error::{Error, Result};
pub use executor::{GeminiRequest, OutputFormat};

use std::path::Path;

/// Default Gemini model
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

/// Options for analysis
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Gemini model to use
    pub model: String,
    /// Output format (text or json)
    pub output_format: OutputFormat,
    /// Custom Gemini CLI path (optional)
    pub gemini_path: Option<String>,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            output_format: OutputFormat::Text,
            gemini_path: None,
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

    /// Set output format to JSON
    pub fn json(mut self) -> Self {
        self.output_format = OutputFormat::Json;
        self
    }

    /// Set custom Gemini CLI path
    pub fn with_gemini_path(mut self, path: impl Into<String>) -> Self {
        self.gemini_path = Some(path.into());
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
/// use gemini_analyzer::{analyze, AnalyzeOptions};
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
    let temp_dir = temp::create_temp_dir("gemini-analyzer")?;
    let result = analyze_in_dir(&temp_dir, prompt, files, options);
    temp::cleanup_temp_dir(&temp_dir);
    result
}

/// Analyze with a prompt only (no files)
///
/// # Arguments
///
/// * `prompt` - The prompt to send to Gemini
/// * `options` - Analysis options
///
/// # Returns
///
/// The response as a string
///
/// # Example
///
/// ```rust,no_run
/// use gemini_analyzer::{prompt, AnalyzeOptions};
///
/// let result = prompt(
///     "Explain the construction document verification process in Japanese",
///     AnalyzeOptions::default(),
/// ).unwrap();
/// ```
pub fn prompt(prompt: &str, options: AnalyzeOptions) -> Result<String> {
    let temp_dir = temp::create_temp_dir("gemini-analyzer")?;
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

    let request = GeminiRequest {
        prompt,
        model: &options.model,
        files: Some(&file_names),
        output_format: options.output_format,
    };

    executor::run_gemini(work_dir, &request, options.gemini_path.as_deref())
}

/// Run a prompt without files in a specific directory
pub fn prompt_in_dir(work_dir: &Path, prompt: &str, options: AnalyzeOptions) -> Result<String> {
    let request = GeminiRequest {
        prompt,
        model: &options.model,
        files: None,
        output_format: options.output_format,
    };

    executor::run_gemini(work_dir, &request, options.gemini_path.as_deref())
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

    /// Set custom Gemini CLI path
    pub fn gemini_path(mut self, path: impl Into<String>) -> Self {
        self.options.gemini_path = Some(path.into());
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
    }

    #[test]
    fn test_analyze_options_builder() {
        let opts = AnalyzeOptions::with_model("gemini-2.0-flash-exp")
            .json()
            .with_gemini_path("/custom/path");

        assert_eq!(opts.model, "gemini-2.0-flash-exp");
        assert!(matches!(opts.output_format, OutputFormat::Json));
        assert_eq!(opts.gemini_path, Some("/custom/path".to_string()));
    }

    #[test]
    fn test_analysis_builder() {
        let builder = AnalysisBuilder::new("test prompt")
            .file("file1.pdf")
            .file("file2.pdf")
            .model("gemini-2.0-flash-exp")
            .json();

        assert_eq!(builder.prompt, "test prompt");
        assert_eq!(builder.files.len(), 2);
        assert_eq!(builder.options.model, "gemini-2.0-flash-exp");
    }
}
