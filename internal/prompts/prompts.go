// Package prompts contains Japanese document verification and estimation prompt templates.
// Ported from the Rust reference implementation (main.rs build_check_prompt /
// build_compare_prompt, estimate.rs GEOMETRY_PROMPT / FILL_PROMPT).
package prompts

import (
	"path/filepath"
	"strings"
)

// CheckBase is the default Japanese document verification prompt.
const CheckBase = `あなたは日本語で回答するアシスタントです。必ず日本語で回答してください。

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
`

// BuildCheckPrompt builds the check prompt, optionally appending a custom instruction.
func BuildCheckPrompt(instruction string) string {
	if instruction == "" {
		return CheckBase
	}
	return CheckBase + "\n## ユーザー指定のチェック項目\n以下の項目も必ず確認してください：\n" + instruction + "\n"
}

// BuildComparePrompt builds the cross-file comparison prompt.
func BuildComparePrompt(files []string, instruction string) string {
	var names []string
	for _, f := range files {
		names = append(names, filepath.Base(f))
	}
	base := `あなたは日本語で回答するアシスタントです。必ず日本語で回答してください。

添付の複数書類を照合し、書類間の整合性をチェックしてください。

## 照合対象ファイル
` + strings.Join(names, "\n") + `

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
`
	if instruction != "" {
		base += "\n## ユーザー指定のチェック項目\n以下の項目も必ず確認してください：\n" + instruction + "\n"
	}
	return base
}

// GeometryPrompt is the rear-view dump truck geometry detection prompt (JSON output).
const GeometryPrompt = `Output ONLY JSON: {"plateBox":[x1,y1,x2,y2], "tailgateTopY": 0.0, "tailgateBottomY": 0.0, "cargoTopY": 0.0} ` +
	`This is a rear view of a dump truck carrying construction debris. ` +
	`plateBox = bounding box of the rear license plate (normalized 0-1, [left,top,right,bottom]). ` +
	`tailgateTopY = Y coordinate (normalized 0-1) of the TOP edge of the tailgate (後板上端/rim). ` +
	`tailgateBottomY = Y coordinate (normalized 0-1) of the BOTTOM edge of the tailgate (後板下端). ` +
	`cargoTopY = Y coordinate (normalized 0-1) of the HIGHEST point of the cargo pile. ` +
	`The tailgate is the flat metal panel at the rear of the truck bed. ` +
	`tailgateTopY < tailgateBottomY < plateBox[3] (top has smaller Y). ` +
	`cargoTopY < tailgateTopY if cargo is heaped above the rim. ` +
	`cargoTopY > tailgateTopY if cargo is below the rim. ` +
	`All coordinates normalized 0.0-1.0.`

// FillPrompt is the fill ratio / packing density estimation prompt (JSON output).
const FillPrompt = `Output ONLY JSON: {"fillRatioL": 0.0, "fillRatioW": 0.0, "packingDensity": 0.0, "reasoning": "..."} ` +
	`This is a rear view of a dump truck carrying construction debris (As殻 = asphalt chunks). ` +
	`Estimate each parameter INDEPENDENTLY: ` +
	`fillRatioL (0.3~0.9): fraction of the bed LENGTH occupied by cargo. ` +
	`Dump trucks are loaded from above; cargo forms a mound that rarely reaches the very front/rear. ` +
	`Full load with cargo touching both ends = 0.85-0.9. Normal load = 0.6-0.8. Light load = 0.4-0.6. ` +
	`fillRatioW (0.5~1.0): fraction of the bed WIDTH covered by cargo at the top surface. ` +
	`Usually 0.8-1.0 since cargo spreads across the width. ` +
	`packingDensity (0.5~0.9): how tightly packed the debris chunks are. ` +
	`As殻 = asphalt pavement chunks (~5cm thick). ` +
	`Large chunks thrown in loosely with visible gaps = 0.5-0.6, moderate = 0.65-0.7, tightly packed = 0.8-0.9.`
