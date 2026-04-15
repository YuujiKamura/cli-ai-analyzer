# cli-ai-analyzer

Gemini CLI の薄いラッパー。ファイル/画像/PDF/テキストを共通の subcommand で解析する。

**Gemini 専用。** 過去のマルチバックエンド (Claude/Codex/Ollama) は削除済み (Rust版は `~/cli-ai-analyzer-rust-archive/` に退避)。subprocess ベースの CLI 制御モデル (ghostty / deckpilot / CP と同系統) に素直に収まるよう Gemini 1本に絞った。

## 使い方

```bash
# テキストだけ
cli-ai-analyzer prompt "契約書と見積書の違いを説明してください"

# ファイル付き解析
cli-ai-analyzer analyze --prompt "この書類の内容を説明" document.pdf
cli-ai-analyzer analyze --prompt "画像の内容を記述" photo.jpg

# 日本語書類チェック
cli-ai-analyzer check document.pdf
cli-ai-analyzer check document.pdf --instruction "金額100万円超の有無"

# 複数書類の照合
cli-ai-analyzer compare contract.pdf estimate.pdf
cli-ai-analyzer compare doc1.pdf doc2.pdf --instruction "工期が一致するか"

# ダンプトラック積載量推定
cli-ai-analyzer estimate dump_truck.jpg

# JSON 出力
cli-ai-analyzer prompt "..." --json
```

## 共通フラグ

| フラグ | 既定値 | 説明 |
|---|---|---|
| `-m, --model` | `gemini-3-flash-preview` | 使う Gemini モデル |
| `--json` | false | JSON 形式の出力を要求 |
| `--cli-path` | (env) | `gemini` CLI のパス上書き |
| `--pay-per-use` | false | Gemini REST API (要 `GEMINI_API_KEY`) で叩く |

### モデル選び

- `gemini-3-flash-preview` (既定) — 速い、コスト低い
- `gemini-3-pro-preview` — 構造化出力プロンプト (黒板OCR+マスタ照合のような記入式テンプレ) で flash が幻覚で `V1=数値`, `has_board=true` を出す罠を回避。代償は遅さ。バッチサイズ1 / 常駐セッションで緩和

```bash
cli-ai-analyzer analyze -p "..." -m gemini-3-pro-preview photo.jpg
```

## 環境変数

| 変数 | 用途 |
|---|---|
| `GEMINI_CMD_PATH` | `gemini` CLI の絶対パス (`--cli-path` と同義、flag優先) |
| `GEMINI_API_KEY` | `--pay-per-use` 時の REST API キー |

## インストール

```bash
go install github.com/YuujiKamura/cli-ai-analyzer@latest
# -> $GOPATH/bin/cli-ai-analyzer
```

ローカルから:

```bash
cd ~/cli-ai-analyzer && go install .
```

## 前提条件

`gemini` CLI が PATH にあり認証済み (`gemini auth`) であること。または `--pay-per-use` + `GEMINI_API_KEY`。

## ライセンス

MIT
