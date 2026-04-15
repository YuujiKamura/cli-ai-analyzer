# cli-ai-analyzer

複数のAI CLIツール（Gemini, Claude, Ollama）を統一インターフェースで呼び出すRustライブラリ。

## なぜ便利か

- **1つのAPIで複数のAIバックエンドを切り替え** - Gemini、Claude、Ollamaを同じAPIで利用可能
- **プロジェクトごとにAIを変えても呼び出しコードは同じ** - バックエンドを変更してもアプリケーションコードの修正が不要
- **新しいCLIツールも簡単に追加可能** - 統一されたトレイト設計により、新しいAIバックエンドを容易に追加できる

## 対応バックエンド

| バックエンド | CLI ツール | ステータス |
|-------------|-----------|-----------|
| **Gemini** | [Gemini CLI](https://github.com/google/gemini-cli) | 対応済み |
| **Claude** | [Claude Code](https://github.com/anthropics/claude-code) | 計画中 |
| **Ollama** | [Ollama](https://ollama.ai/) | 計画中 |

## インストール

### ライブラリとして

`Cargo.toml` に追加:

```toml
[dependencies]
cli-ai-analyzer = { path = "../cli-ai-analyzer" }
```

### CLIツールとして

```bash
cargo install --path .
```

## 使用例

### Backend切り替え

```rust
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};
use std::path::PathBuf;

// Gemini を使用
let result = analyze(
    "この書類の内容を説明してください",
    &[PathBuf::from("document.pdf")],
    AnalyzeOptions::default(), // デフォルトは Gemini
)?;

// 将来的には Backend を切り替え可能に
// let options = AnalyzeOptions::default()
//     .with_backend(Backend::Claude)
//     .with_model("claude-3-opus");
```

### 基本的な解析

```rust
use cli_ai_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

let result = analyze(
    "この書類の内容は？",
    &[PathBuf::from("document.pdf")],
    AnalyzeOptions::default(),
)?;
println!("{}", result);
```

### テキストのみのプロンプト

```rust
use cli_ai_analyzer::{prompt, AnalyzeOptions};

let result = prompt(
    "建設書類の検証プロセスを説明してください",
    AnalyzeOptions::default(),
)?;
```

### Builderパターン

```rust
use cli_ai_analyzer::AnalysisBuilder;

let result = AnalysisBuilder::new("これらの書類を比較してください")
    .file("doc1.pdf")
    .file("doc2.pdf")
    .model("gemini-2.5-flash")
    .run()?;
```

## CLI使用方法

### ファイル解析

```bash
# 基本的な解析
cli-ai-analyzer analyze --prompt "この書類の内容を説明してください" document.pdf

# 複数ファイル
cli-ai-analyzer analyze --prompt "これらの書類を要約してください" doc1.pdf doc2.pdf

# モデル指定
cli-ai-analyzer analyze --prompt "..." --model gemini-2.0-flash-exp document.pdf

# JSON出力
cli-ai-analyzer analyze --prompt "..." --json document.pdf
```

### テキストのみのプロンプト

```bash
cli-ai-analyzer prompt "契約書と見積書の違いを説明してください"
```

### 書類チェック（日本語）

```bash
# 単一書類のチェック
cli-ai-analyzer check document.pdf

# カスタム指示付き
cli-ai-analyzer check document.pdf --instruction "金額が100万円を超えていないか確認"
```

### 複数書類の照合

```bash
# 整合性チェック
cli-ai-analyzer compare contract.pdf estimate.pdf

# カスタム指示付き
cli-ai-analyzer compare doc1.pdf doc2.pdf --instruction "工期が一致しているか確認"
```

## API リファレンス

### 主要関数

| 関数 | 説明 |
|------|------|
| `analyze(prompt, files, options)` | ファイル付きで解析を実行 |
| `prompt(prompt, options)` | テキストのみで解析を実行 |
| `analyze_in_dir(dir, prompt, files, options)` | 指定ディレクトリで解析（上級者向け） |

### AnalyzeOptions

| フィールド | 型 | デフォルト | 説明 |
|-----------|------|---------|------|
| `model` | String | `gemini-2.5-flash` | 使用するモデル |
| `output_format` | OutputFormat | Text | 出力形式（Text/Json） |
| `gemini_path` | Option<String> | None | カスタムCLIパス |

## 前提条件

使用するバックエンドに応じて、対応するCLIツールをインストール・認証してください：

- **Gemini**: [Gemini CLI](https://github.com/google/gemini-cli) をインストール・認証
- **Claude**: Claude Code をインストール・認証（計画中）
- **Ollama**: Ollama をインストール・起動（計画中）

## 環境変数

- `GEMINI_CMD_PATH` - Gemini CLI実行ファイルのカスタムパス

## ライセンス

MIT
