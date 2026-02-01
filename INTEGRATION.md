# Integration Guide

How to integrate gemini-analyzer into your projects.

## ShoruiChecker Integration

### Step 1: Add dependency

In `ShoruiChecker/src-tauri/Cargo.toml`:

```toml
[dependencies]
gemini-analyzer = { path = "../../gemini-analyzer" }
```

### Step 2: Replace gemini_cli.rs usage

Before (in `analysis.rs`):
```rust
use crate::gemini_cli::{create_temp_dir, cleanup_temp_dir, run_gemini_with_prompt};

// Old code
let temp_dir = create_temp_dir(&format!(".shoruichecker_temp_{}", task_id))?;
let output = run_gemini_with_prompt(&temp_dir, &prompt, model, Some(&pdfs));
cleanup_temp_dir(&temp_dir);
```

After:
```rust
use gemini_analyzer::{analyze, AnalyzeOptions};

// New code
let options = AnalyzeOptions::with_model(model);
let output = analyze(&prompt, &pdf_paths, options);
```

### Step 3: Keep ShoruiChecker-specific logic

Keep these in ShoruiChecker:
- `guidelines.rs` - Document type detection and guidelines
- `history.rs` - Analysis history management
- `pdf_embed.rs` - PDF metadata embedding
- `settings.rs` - Application settings

Only move to gemini-analyzer:
- Gemini CLI execution
- Temporary directory management
- Basic prompt/response handling

## photo-ai-rust Integration

### Step 1: Add dependency

In `photo-ai-rust/Cargo.toml`:

```toml
[dependencies]
gemini-analyzer = { path = "../gemini-analyzer" }
```

### Step 2: Use for image analysis

```rust
use gemini_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

pub fn analyze_photo(image_path: &str, prompt: &str, model: &str) -> Result<String, String> {
    let options = AnalyzeOptions::with_model(model);
    analyze(prompt, &[PathBuf::from(image_path)], options)
        .map_err(|e| e.to_string())
}
```

## SekouTaiseiMaker Integration

SekouTaiseiMaker uses WASM and calls Gemini API directly (not via CLI).
For WASM projects, consider:

1. Keep the direct API approach for browser compatibility
2. Use gemini-analyzer for native/CLI operations only
3. Share prompt templates between both implementations

### Shared Prompts

Create a shared prompts module:

```rust
// In a shared crate or copy to both projects
pub mod prompts {
    pub fn get_document_check_prompt(doc_type: &str) -> String {
        match doc_type {
            "暴対法誓約書" => include_str!("prompts/bouseihou.txt").to_string(),
            "作業員名簿" => include_str!("prompts/sagyouin.txt").to_string(),
            _ => get_generic_prompt(doc_type),
        }
    }
}
```

## API Differences

### gemini-analyzer (CLI-based)
- Uses Gemini CLI (`gemini` command)
- Requires CLI installation and authentication
- Works on desktop/native platforms
- Handles files directly via filesystem

### Direct API (SekouTaiseiMaker)
- Uses Gemini REST API directly
- Requires API key
- Works in browser/WASM
- Files sent as base64 encoded data

## Migration Strategy

1. **Phase 1**: Use gemini-analyzer alongside existing code
2. **Phase 2**: Gradually migrate file analysis to gemini-analyzer
3. **Phase 3**: Remove old gemini_cli.rs code
4. **Phase 4**: Share prompts between projects

## Error Handling

Map gemini-analyzer errors to your project's error type:

```rust
use gemini_analyzer::Error as GeminiError;

impl From<GeminiError> for MyAppError {
    fn from(e: GeminiError) -> Self {
        match e {
            GeminiError::GeminiCliNotFound => MyAppError::DependencyMissing("Gemini CLI"),
            GeminiError::AuthenticationFailed => MyAppError::AuthRequired("Gemini"),
            GeminiError::Io(e) => MyAppError::Io(e),
            _ => MyAppError::Analysis(e.to_string()),
        }
    }
}
```

## Testing

Test with mock or real Gemini CLI:

```rust
#[cfg(test)]
mod tests {
    use gemini_analyzer::{prompt, AnalyzeOptions};

    #[test]
    #[ignore] // Requires Gemini CLI
    fn test_real_prompt() {
        let result = prompt("Say 'hello'", AnalyzeOptions::default());
        assert!(result.is_ok());
    }
}
```
