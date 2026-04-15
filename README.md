# cli-ai-analyzer

AI CLI ツール（Gemini / Claude）を統一インターフェースで呼び出す Rust ライブラリ + CLI。

## なぜ便利か

- **1つのAPIで複数のAIバックエンドを切り替え** — Gemini と Claude を同じ呼び出しコードで利用可能
- **プロジェクトごとに AI を変えても呼び出しコードは同じ** — バックエンドを変更しても利用側の修正は不要
- **ファイル付き解析 / プロンプトのみ / 書類チェック / 比較** をサブコマンドで提供

## 対応バックエンド（実態）

| バックエンド | CLI ツール | ライブラリ | CLI 経由 | 備考 |
|-------------|-----------|-----------|---------|------|
| **Gemini** | [Gemini CLI](https://github.com/google/gemini-cli) | ✅ | ✅ | デフォルト。`DEFAULT_MODEL = gemini-3-pro-preview` (2026-04-14更新)。`--pay-per-use` で Gemini REST API (`GEMINI_API_KEY`) も選択可 |
| **Claude** | [Claude Code](https://github.com/anthropics/claude-code) | ✅ | ✅ | `DEFAULT_CLAUDE_MODEL = claude-sonnet-4-20250514` |
| **Codex** | [Codex CLI](https://github.com/openai/codex) | ✅ | ❌ | `Backend::Codex` はライブラリで実装済み。ただし CLI の `--backend` 引数は現在 `gemini` / `claude` のみ受け付ける（`main.rs::BackendArg` が2択のため） |
| **Ollama** | [Ollama](https://ollama.ai/) | ⚠️ スタブのみ | ❌ | `Backend::Ollama` は enum に存在するが、実行時に `"Ollama backend is not yet implemented"` エラーを返すだけで未実装 |

## インストール

### ライブラリとして

`Cargo.toml` に追加:

```toml
[dependencies]
cli-ai-analyzer = { path = "../cli-ai-analyzer" }
```

### CLI ツールとして

```bash
cargo install --path .
# → $CARGO_HOME/bin/cli-ai-analyzer
```

## CLI 使用方法

```
Usage: cli-ai-analyzer.exe <COMMAND>

Commands:
  analyze   Analyze files with a prompt
  prompt    Send a text-only prompt (no files)
  check     Check files with a document verification prompt (Japanese)
  estimate  Estimate dump truck cargo weight from image
  compare   Compare multiple files for consistency
```

### ファイル解析

```bash
# 基本
cli-ai-analyzer analyze --prompt "この書類の内容を説明してください" document.pdf

# 複数ファイル
cli-ai-analyzer analyze --prompt "これらの書類を要約してください" doc1.pdf doc2.pdf

# モデル指定（現行世代の例）
cli-ai-analyzer analyze --prompt "..." --model gemini-3-pro-preview document.pdf

# Claude バックエンド
cli-ai-analyzer analyze --prompt "..." --backend claude document.pdf

# JSON出力
cli-ai-analyzer analyze --prompt "..." --json document.pdf
```

### テキストのみプロンプト

```bash
cli-ai-analyzer prompt "契約書と見積書の違いを説明してください"

# Pay-per-use (Gemini REST API、GEMINI_API_KEY 環境変数が必要)
cli-ai-analyzer prompt "..." --pay-per-use
```

### 書類チェック（日本語）

```bash
cli-ai-analyzer check document.pdf
cli-ai-analyzer check document.pdf --instruction "金額が100万円を超えていないか確認"
```

### 複数書類の照合

```bash
cli-ai-analyzer compare contract.pdf estimate.pdf
cli-ai-analyzer compare doc1.pdf doc2.pdf --instruction "工期が一致しているか確認"
```

## ライブラリ使用例

### デフォルト (Gemini)

```rust
use cli_ai_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

let result = analyze(
    "この書類の内容を説明してください",
    &[PathBuf::from("document.pdf")],
    AnalyzeOptions::default(),
)?;
```

### Backend 切り替え

```rust
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};

let options = AnalyzeOptions::default()
    .with_backend(Backend::Claude)
    .with_model("claude-sonnet-4-20250514");

let result = analyze(
    "この書類を要約",
    &[PathBuf::from("document.pdf")],
    options,
)?;
```

### プロンプトのみ

```rust
use cli_ai_analyzer::{prompt, AnalyzeOptions};

let result = prompt(
    "建設書類の検証プロセスを説明してください",
    AnalyzeOptions::default(),
)?;
```

### Builder パターン

```rust
use cli_ai_analyzer::AnalysisBuilder;

let result = AnalysisBuilder::new("これらの書類を比較してください")
    .file("doc1.pdf")
    .file("doc2.pdf")
    .model("gemini-3-pro-preview")
    .run()?;
```

## API リファレンス

### 主要関数

| 関数 | 説明 |
|------|------|
| `analyze(prompt, files, options)` | ファイル付きで解析を実行 |
| `prompt(prompt, options)` | テキストのみで解析を実行 |
| `analyze_in_dir(dir, prompt, files, options)` | 指定ディレクトリで解析（上級者向け） |

### 主要型

| 型 | 説明 |
|------|------|
| `AnalyzeOptions` | 解析時の設定（backend / model / output_format / CLI パス等） |
| `AnalysisBuilder` | ファイル追加＋オプション設定を連鎖させる builder |
| `Backend` | `Gemini` / `Claude` / `Codex` / `Ollama` （Ollama はスタブ） |
| `OutputFormat` | `Text` / `Json` |

### デフォルトモデル（定数）

```rust
pub const DEFAULT_MODEL: &str = "gemini-3-pro-preview";
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";
```

## 前提条件

使用するバックエンドに応じて、対応する CLI ツールをインストール・認証してください:

- **Gemini**: [Gemini CLI](https://github.com/google/gemini-cli) をインストール・認証（または `GEMINI_API_KEY` を設定して `--pay-per-use`）
- **Claude**: [Claude Code](https://github.com/anthropics/claude-code) をインストール・認証

## 環境変数

| 変数 | 用途 |
|------|------|
| `GEMINI_CMD_PATH` | Gemini CLI 実行ファイルのカスタムパス |
| `CLAUDE_CMD_PATH` | Claude CLI 実行ファイルのカスタムパス |
| `CODEX_CMD_PATH` | Codex CLI 実行ファイルのカスタムパス（ライブラリ経由のみ） |
| `GEMINI_API_KEY` | `--pay-per-use` 時の Gemini REST API キー |

## 既知の制約

- **Codex backend は CLI 未対応**: ライブラリ (`Backend::Codex`) では実装済みだが、CLI の `--backend` 引数は `gemini` / `claude` のみ受け付ける。Codex を使うにはライブラリ経由で呼び出す必要がある。
- **Ollama backend は未実装**: `Backend::Ollama` を指定すると実行時エラー。enum バリアントのみ残っている。
- **`gemini-3-pro-preview` は Flash より遅い**: 構造化出力の幻覚を避けるため 2026-04-14 に Pro へ切り替え (`371183e`)。バッチサイズ1 か ACP 常駐セッションで緩和する想定。
- **Claude backend のスモークテスト未完**: 実装経路 (`Backend::Claude` の executor 分岐と CLI `--backend claude`) は存在。ただし `prompt "Reply with only the word PONG" --backend claude` のスモークテストが4分以上無応答のため、コールドスタート起因か接続経路の不具合かは未切り分け。Gemini 経路は同条件でPING応答確認済み。

## 実地確認ステータス (2026-04-15)

| 確認項目 | 結果 |
|------|------|
| `cli-ai-analyzer prompt ... --backend gemini` | ✅ PING応答 (`gemini-3-pro-preview`) |
| `cli-ai-analyzer prompt ... --backend claude` | ⏳ 4分+無応答で打ち切り、要調査 |
| Codex / Ollama | CLI 経由では検証不能 |

## ライセンス

MIT
