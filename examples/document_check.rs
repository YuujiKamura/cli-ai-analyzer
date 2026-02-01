//! Document verification example (Japanese construction documents)
//!
//! Run with: cargo run --example document_check -- document.pdf

use cli_ai_analyzer::{analyze, AnalyzeOptions};
use std::env;
use std::path::PathBuf;

/// Build a document verification prompt
fn build_verification_prompt(custom_instruction: Option<&str>) -> String {
    let base = r#"あなたは日本語で回答するアシスタントです。必ず日本語で回答してください。

添付のPDF書類の内容を読み取り、整合性をチェックしてください。

## 注意事項
- 文字は正確に読み取ること（特に地名、人名、会社名）
- 似た漢字を間違えないこと
- 数値は桁を間違えないこと

## 書類タイプ別チェックポイント

### 契約書の場合
- 契約当事者（発注者・受注者）の名称が書類内で一貫しているか
- 金額計算（工事価格 + 消費税 = 請負代金額）が正しいか
- 工期の日付が妥当か（着工日 < 完成日）
- 必要な署名・押印欄があるか
- 選択肢形式の項目は○（丸）がついている選択肢を読み取ること

### 見積書の場合
- 宛先が正しく記入されているか
- 見積金額の合計が正しいか
- 有効期限が記載されているか

### 請求書の場合
- 請求先が正しいか
- 金額計算が正しいか
- 振込先情報が記載されているか

## 出力形式
- まず書類タイプを判定して報告
- 整合している項目は「✓」で示す
- 問題がある項目は「⚠」で具体的に指摘
"#;

    if let Some(instruction) = custom_instruction {
        format!(
            "{}\n## ユーザー指定のチェック項目\n以下の項目も必ず確認してください：\n{}\n",
            base, instruction
        )
    } else {
        base.to_string()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run --example document_check -- <pdf_file> [custom_instruction]");
        std::process::exit(1);
    }

    let file = PathBuf::from(&args[1]);
    let custom_instruction = args.get(2).map(|s| s.as_str());

    if !file.exists() {
        eprintln!("File not found: {}", file.display());
        std::process::exit(1);
    }

    println!("Checking document: {}", file.display());
    if let Some(inst) = custom_instruction {
        println!("Custom instruction: {}", inst);
    }
    println!();

    let prompt = build_verification_prompt(custom_instruction);

    match analyze(&prompt, &[file], AnalyzeOptions::default()) {
        Ok(result) => {
            println!("=== 検証結果 ===\n");
            println!("{}", result);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
