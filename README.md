# gemini-analyzer

A universal AI analysis library and CLI powered by Gemini CLI.

## Features

- **Library API** - Use as a Rust crate in your projects
- **CLI Tool** - Analyze files from the command line
- **File Support** - PDF, images, text files
- **Japanese Document Verification** - Built-in prompts for checking Japanese documents
- **Multiple File Comparison** - Compare documents for consistency

## Installation

### As a CLI tool

```bash
cargo install --path .
```

### As a library

Add to your `Cargo.toml`:

```toml
[dependencies]
gemini-analyzer = { path = "../gemini-analyzer" }
```

## Prerequisites

- [Gemini CLI](https://github.com/google/gemini-cli) installed and authenticated
- On Windows: `gemini.cmd` in PATH
- On Unix: `gemini` in PATH

## CLI Usage

### Analyze files

```bash
# Basic analysis
gemini-analyzer analyze --prompt "Describe this document" document.pdf

# Multiple files
gemini-analyzer analyze --prompt "Summarize these documents" doc1.pdf doc2.pdf

# With specific model
gemini-analyzer analyze --prompt "..." --model gemini-2.0-flash-exp document.pdf

# JSON output
gemini-analyzer analyze --prompt "..." --json document.pdf
```

### Text-only prompts

```bash
gemini-analyzer prompt "Explain the difference between 契約書 and 見積書"
```

### Document verification (Japanese)

```bash
# Check a single document
gemini-analyzer check document.pdf

# With custom instructions
gemini-analyzer check document.pdf --instruction "金額が100万円を超えていないか確認"
```

### Compare multiple documents

```bash
# Compare for consistency
gemini-analyzer compare contract.pdf estimate.pdf

# With custom instructions
gemini-analyzer compare doc1.pdf doc2.pdf --instruction "工期が一致しているか確認"
```

## Library Usage

### Basic analysis

```rust
use gemini_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

let result = analyze(
    "What is in this document?",
    &[PathBuf::from("document.pdf")],
    AnalyzeOptions::default(),
)?;
println!("{}", result);
```

### Text-only prompt

```rust
use gemini_analyzer::{prompt, AnalyzeOptions};

let result = prompt(
    "Explain construction documents in Japanese",
    AnalyzeOptions::default(),
)?;
```

### With options

```rust
use gemini_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

let options = AnalyzeOptions::with_model("gemini-2.0-flash-exp")
    .json()
    .with_gemini_path("/custom/path/gemini");

let result = analyze(
    "Analyze this document",
    &[PathBuf::from("doc.pdf")],
    options,
)?;
```

### Builder pattern

```rust
use gemini_analyzer::AnalysisBuilder;

let result = AnalysisBuilder::new("Compare these documents")
    .file("doc1.pdf")
    .file("doc2.pdf")
    .model("gemini-2.5-flash")
    .run()?;
```

## Environment Variables

- `GEMINI_CMD_PATH` - Custom path to Gemini CLI executable

## Error Handling

The library provides specific error types:

```rust
use gemini_analyzer::{analyze, Error, AnalyzeOptions};
use std::path::PathBuf;

match analyze("...", &[PathBuf::from("doc.pdf")], AnalyzeOptions::default()) {
    Ok(result) => println!("{}", result),
    Err(Error::GeminiCliNotFound) => {
        eprintln!("Please install Gemini CLI: npm install -g @google/gemini-cli");
    }
    Err(Error::AuthenticationFailed) => {
        eprintln!("Please run: gemini auth");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Integration with Other Projects

This crate is designed to be used by:

- **ShoruiChecker** - PDF document verification
- **photo-ai-rust** - Construction photo analysis
- **SekouTaiseiMaker** - Construction document management

## License

MIT
