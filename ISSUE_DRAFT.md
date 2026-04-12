# PowerShell Start-Process parameter error in Gemini CLI calls

## 問題
レジデントエーAI呼び出し切り替え後、Gemini CLI呼び出し時にPowerShellパラメーターエラーが発生

## エラーメッセージ
```
Start-Process : パラメーター 'm' は、位置指定パラメーターを受け入れないため、解決できません。
発生場所 行:1 文字:1
+ Start-Process -FilePath 'gemini.cmd' -ArgumentList '-m gemini-3-flash ...
```

## 再現手順
1. skill-miner実行時にGemini CLI呼び出しが発生
2. cli-ai-analyzer経由でGemini CLIが呼ばれる
3. PowerShellのStart-Processでパラメーター渡しが失敗

## 原因
executor.rs のGemini CLI呼び出し部分で、PowerShell引数の構築方法に問題があると推測

## 影響範囲
- skill-miner の今日の活動要約機能が動作不能
- Gemini CLI を使用する他のツールも影響を受ける可能性

## 環境
- Windows 11 Pro 10.0.26200
- PowerShell経由でのGemini CLI呼び出し

## 修正が必要なファイル
- src/executor.rs (Gemini CLI呼び出し部分)