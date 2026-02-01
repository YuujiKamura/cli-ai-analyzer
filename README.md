# gemini-analyzer

Gemini CLIをラップした汎用AIファイル解析ライブラリ＆CLIツール。

## なぜこれが便利か

- **Gemini CLIの複雑さを隠蔽**: 一時ディレクトリ管理、スクリプト生成、エラーハンドリングを自動化
- **複数プロジェクトで共通利用**: ShoruiChecker, photo-ai-rust, SekouTaiseiMaker など各プロジェクトから同じAPIで呼び出せる
- **日本語書類チェックに特化したプロンプト内蔵**: 契約書・見積書の照合、整合性確認がすぐにできる
- **Windows/Unix両対応**: OSに応じたスクリプト生成を自動で行う

## ユースケース

### ShoruiChecker での利用
PDF書類の内容チェック、複数書類の照合に使用。Gemini CLIを直接呼び出すコードをこのクレートに置き換え可能。

### photo-ai-rust での利用
工事写真のAI解析に使用。写真の内容説明、工事進捗の判定などに活用。

### SekouTaiseiMaker での利用
施工体制台帳関連書類の生成・検証に使用（ネイティブ版）。

## 機能

- **ライブラリAPI** - Rustクレートとしてプロジェクトに組み込み
- **CLIツール** - コマンドラインからファイル解析
- **ファイル対応** - PDF、画像、テキストファイル
- **日本語書類チェック** - 内蔵プロンプトで書類の整合性確認
- **複数ファイル比較** - 書類間の整合性チェック

## インストール

### CLIツールとして

```bash
cargo install --path .
```

### ライブラリとして

`Cargo.toml` に追加:

```toml
[dependencies]
gemini-analyzer = { path = "../gemini-analyzer" }
```

## 前提条件

- [Gemini CLI](https://github.com/google/gemini-cli) がインストール・認証済み
- Windows: `gemini.cmd` がPATHに存在
- Unix: `gemini` がPATHに存在

## CLI使用方法

### ファイル解析

```bash
# 基本的な解析
gemini-analyzer analyze --prompt "この書類の内容を説明してください" document.pdf

# 複数ファイル
gemini-analyzer analyze --prompt "これらの書類を要約してください" doc1.pdf doc2.pdf

# モデル指定
gemini-analyzer analyze --prompt "..." --model gemini-2.0-flash-exp document.pdf

# JSON出力
gemini-analyzer analyze --prompt "..." --json document.pdf
```

### テキストのみのプロンプト

```bash
gemini-analyzer prompt "契約書と見積書の違いを説明してください"
```

### 書類チェック（日本語）

```bash
# 単一書類のチェック
gemini-analyzer check document.pdf

# カスタム指示付き
gemini-analyzer check document.pdf --instruction "金額が100万円を超えていないか確認"
```

### 複数書類の照合

```bash
# 整合性チェック
gemini-analyzer compare contract.pdf estimate.pdf

# カスタム指示付き
gemini-analyzer compare doc1.pdf doc2.pdf --instruction "工期が一致しているか確認"
```

## ライブラリ使用方法

### 基本的な解析

```rust
use gemini_analyzer::{analyze, AnalyzeOptions};
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
use gemini_analyzer::{prompt, AnalyzeOptions};

let result = prompt(
    "建設書類の検証プロセスを日本語で説明してください",
    AnalyzeOptions::default(),
)?;
```

### オプション指定

```rust
use gemini_analyzer::{analyze, AnalyzeOptions};
use std::path::PathBuf;

let options = AnalyzeOptions::with_model("gemini-2.0-flash-exp")
    .json()
    .with_gemini_path("/custom/path/gemini");

let result = analyze(
    "この書類を解析してください",
    &[PathBuf::from("doc.pdf")],
    options,
)?;
```

### Builderパターン

```rust
use gemini_analyzer::AnalysisBuilder;

let result = AnalysisBuilder::new("これらの書類を比較してください")
    .file("doc1.pdf")
    .file("doc2.pdf")
    .model("gemini-2.5-flash")
    .run()?;
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
| `model` | String | `gemini-2.5-flash` | 使用するGeminiモデル |
| `output_format` | OutputFormat | Text | 出力形式（Text/Json） |
| `gemini_path` | Option<String> | None | カスタムGemini CLIパス |

### エラー型

| エラー | 説明 |
|--------|------|
| `GeminiCliNotFound` | Gemini CLIが見つからない |
| `AuthenticationFailed` | 認証が必要 |
| `InvalidPath` | 無効なファイルパス |
| `FileNotFound` | ファイルが存在しない |
| `GeminiError` | Gemini CLIの実行エラー |

## 環境変数

- `GEMINI_CMD_PATH` - Gemini CLI実行ファイルのカスタムパス

## エラーハンドリング

```rust
use gemini_analyzer::{analyze, Error, AnalyzeOptions};
use std::path::PathBuf;

match analyze("...", &[PathBuf::from("doc.pdf")], AnalyzeOptions::default()) {
    Ok(result) => println!("{}", result),
    Err(Error::GeminiCliNotFound) => {
        eprintln!("Gemini CLIをインストールしてください: npm install -g @google/gemini-cli");
    }
    Err(Error::AuthenticationFailed) => {
        eprintln!("認証を実行してください: gemini auth");
    }
    Err(e) => eprintln!("エラー: {}", e),
}
```

## 連携プロジェクト

| プロジェクト | 用途 |
|------------|------|
| **ShoruiChecker** | PDF書類の検証・チェック |
| **photo-ai-rust** | 工事写真のAI解析 |
| **SekouTaiseiMaker** | 施工体制台帳の書類管理 |

詳細は [INTEGRATION.md](INTEGRATION.md) を参照。

## ライセンス

MIT
