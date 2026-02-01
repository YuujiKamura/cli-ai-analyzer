//! gemini-analyzer CLI
//!
//! A command-line interface for AI analysis using Gemini CLI.
//!
//! ## Usage
//!
//! ```bash
//! # Analyze files with a prompt
//! gemini-analyzer analyze --prompt "Describe this document" document.pdf
//!
//! # Analyze multiple files
//! gemini-analyzer analyze --prompt "Compare these documents" doc1.pdf doc2.pdf
//!
//! # Text-only prompt
//! gemini-analyzer prompt "Explain construction document verification"
//!
//! # Use a specific model
//! gemini-analyzer analyze --prompt "..." --model gemini-2.0-flash-exp document.pdf
//!
//! # JSON output
//! gemini-analyzer analyze --prompt "..." --json document.pdf
//! ```

use clap::{Parser, Subcommand};
use gemini_analyzer::{analyze, prompt, AnalyzeOptions, OutputFormat, DEFAULT_MODEL};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "gemini-analyzer",
    version,
    about = "AI analysis CLI powered by Gemini",
    long_about = "A universal AI analysis tool using Gemini CLI.\n\n\
                  Supports PDF, image, and text file analysis with customizable prompts."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze files with a prompt
    #[command(alias = "a")]
    Analyze {
        /// The prompt to send to Gemini
        #[arg(short, long)]
        prompt: String,

        /// Files to analyze (PDF, images, etc.)
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Gemini model to use
        #[arg(short, long, default_value = DEFAULT_MODEL)]
        model: String,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Custom Gemini CLI path
        #[arg(long, env = "GEMINI_CMD_PATH")]
        gemini_path: Option<String>,
    },

    /// Send a text-only prompt (no files)
    #[command(alias = "p")]
    Prompt {
        /// The prompt to send
        prompt: String,

        /// Gemini model to use
        #[arg(short, long, default_value = DEFAULT_MODEL)]
        model: String,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Custom Gemini CLI path
        #[arg(long, env = "GEMINI_CMD_PATH")]
        gemini_path: Option<String>,
    },

    /// Check files with a document verification prompt (Japanese)
    #[command(alias = "c")]
    Check {
        /// Files to check
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Custom verification instructions (appended to default prompt)
        #[arg(short, long)]
        instruction: Option<String>,

        /// Gemini model to use
        #[arg(short, long, default_value = DEFAULT_MODEL)]
        model: String,

        /// Custom Gemini CLI path
        #[arg(long, env = "GEMINI_CMD_PATH")]
        gemini_path: Option<String>,
    },

    /// Compare multiple files for consistency
    #[command(alias = "cmp")]
    Compare {
        /// Files to compare
        #[arg(required = true, num_args = 2..)]
        files: Vec<PathBuf>,

        /// Custom comparison instructions
        #[arg(short, long)]
        instruction: Option<String>,

        /// Gemini model to use
        #[arg(short, long, default_value = DEFAULT_MODEL)]
        model: String,

        /// Custom Gemini CLI path
        #[arg(long, env = "GEMINI_CMD_PATH")]
        gemini_path: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Analyze {
            prompt: prompt_text,
            files,
            model,
            json,
            gemini_path,
        } => {
            // Validate files exist
            for file in &files {
                if !file.exists() {
                    eprintln!("Error: File not found: {}", file.display());
                    return ExitCode::from(1);
                }
            }

            let mut options = AnalyzeOptions::with_model(model);
            if json {
                options.output_format = OutputFormat::Json;
            }
            if let Some(path) = gemini_path {
                options.gemini_path = Some(path);
            }

            analyze(&prompt_text, &files, options)
        }

        Commands::Prompt {
            prompt: prompt_text,
            model,
            json,
            gemini_path,
        } => {
            let mut options = AnalyzeOptions::with_model(model);
            if json {
                options.output_format = OutputFormat::Json;
            }
            if let Some(path) = gemini_path {
                options.gemini_path = Some(path);
            }

            prompt(&prompt_text, options)
        }

        Commands::Check {
            files,
            instruction,
            model,
            gemini_path,
        } => {
            // Validate files exist
            for file in &files {
                if !file.exists() {
                    eprintln!("Error: File not found: {}", file.display());
                    return ExitCode::from(1);
                }
            }

            let prompt_text = build_check_prompt(&instruction);
            let mut options = AnalyzeOptions::with_model(model);
            if let Some(path) = gemini_path {
                options.gemini_path = Some(path);
            }

            analyze(&prompt_text, &files, options)
        }

        Commands::Compare {
            files,
            instruction,
            model,
            gemini_path,
        } => {
            // Validate files exist
            for file in &files {
                if !file.exists() {
                    eprintln!("Error: File not found: {}", file.display());
                    return ExitCode::from(1);
                }
            }

            let prompt_text = build_compare_prompt(&files, &instruction);
            let mut options = AnalyzeOptions::with_model(model);
            if let Some(path) = gemini_path {
                options.gemini_path = Some(path);
            }

            analyze(&prompt_text, &files, options)
        }
    };

    match result {
        Ok(output) => {
            println!("{}", output);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(1)
        }
    }
}

/// Build a document verification prompt (Japanese)
fn build_check_prompt(custom_instruction: &Option<String>) -> String {
    let base_prompt = r#"あなたは日本語で回答するアシスタントです。必ず日本語で回答してください。

添付の書類の内容を読み取り、整合性をチェックしてください。

## 注意事項
- 文字は正確に読み取ること（特に地名、人名、会社名）
- 似た漢字を間違えないこと
- 数値は桁を間違えないこと

## 一般的なチェックポイント
- 日付が記入されているか
- 必要な署名・記名欄が埋まっているか
- 金額計算が正しいか
- 数量・単価の整合性

## 出力形式
- まず書類タイプを判定して報告
- 整合している項目は「✓」で示す
- 問題がある項目は「⚠」で具体的に指摘
"#;

    if let Some(instruction) = custom_instruction {
        format!(
            "{}\n## ユーザー指定のチェック項目\n以下の項目も必ず確認してください：\n{}\n",
            base_prompt, instruction
        )
    } else {
        base_prompt.to_string()
    }
}

/// Build a comparison prompt for multiple files
fn build_compare_prompt(files: &[PathBuf], custom_instruction: &Option<String>) -> String {
    let file_names: Vec<String> = files
        .iter()
        .filter_map(|f| f.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect();

    let base_prompt = format!(
        r#"あなたは日本語で回答するアシスタントです。必ず日本語で回答してください。

添付の複数書類を照合し、書類間の整合性をチェックしてください。

## 照合対象ファイル
{}

## チェックポイント
- 書類間で当事者名（発注者・受注者・会社名）が一致しているか
- 金額が書類間で整合しているか（見積書と契約書の金額一致等）
- 日付の整合性（契約日、工期、納期等）
- 数量・単価の整合性
- 印影・署名の有無

## 出力形式
1. 各書類の概要を簡潔に説明
2. 書類間で整合している項目は「✓」で示す
3. 不整合や矛盾がある項目は「⚠」で具体的に指摘
4. 総合判定（整合/要確認/不整合）
"#,
        file_names.join("\n")
    );

    if let Some(instruction) = custom_instruction {
        format!(
            "{}\n## ユーザー指定のチェック項目\n以下の項目も必ず確認してください：\n{}\n",
            base_prompt, instruction
        )
    } else {
        base_prompt
    }
}
