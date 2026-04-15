package cmd

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"sort"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/YuujiKamura/cli-ai-analyzer/internal/prompts"
	"github.com/spf13/cobra"
)

// shapeFactor corrects for mound-shaped cargo (empirical ~0.85).
const shapeFactor = 0.85

type truckSpec struct {
	bedLength float64
	bedWidth  float64
	bedHeight float64
}

var truckSpecs = map[string]truckSpec{
	"2t":    {3.0, 1.6, 0.32},
	"4t":    {3.4, 2.06, 0.32},
	"増トン": {4.0, 2.2, 0.40},
	"10t":   {5.3, 2.3, 0.50},
}

var materialDensity = map[string]float64{
	"土砂":      1.8,
	"As殻":     2.5,
	"Co殻":     2.5,
	"開粒度As殻": 2.35,
}

// EstimateResult mirrors the Rust EstimateResult for JSON output.
type EstimateResult struct {
	Method           string  `json:"method"`
	TruckClass       string  `json:"truck_class"`
	MaterialType     string  `json:"material_type"`
	HeightM          float64 `json:"height_m"`
	FillRatioL       float64 `json:"fill_ratio_l"`
	FillRatioW       float64 `json:"fill_ratio_w"`
	PackingDensity   float64 `json:"packing_density"`
	EstimatedVolumeM3 float64 `json:"estimated_volume_m3"`
	EstimatedTonnage float64 `json:"estimated_tonnage"`
	Density          float64 `json:"density"`
	Reasoning        string  `json:"reasoning"`
}

// NewEstimateCmd returns the 'estimate' subcommand.
func NewEstimateCmd() *cobra.Command {
	var truckClass string
	var material string
	var ensemble int
	var cf commonFlags

	c := &cobra.Command{
		Use:     "estimate [flags] <image>",
		Aliases: []string{"e"},
		Short:   "Estimate dump truck cargo weight from image",
		Args:    cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			imgPath := args[0]
			if _, err := os.Stat(imgPath); err != nil {
				return fmt.Errorf("file not found: %s", imgPath)
			}

			spec, ok := truckSpecs[truckClass]
			if !ok {
				return fmt.Errorf("unknown truck class %q (valid: 2t, 4t, 増トン, 10t)", truckClass)
			}
			density, ok := materialDensity[material]
			if !ok {
				return fmt.Errorf("unknown material %q (valid: 土砂, As殻, Co殻, 開粒度As殻)", material)
			}

			makeReq := func(prompt string, jsonMode bool) *gemini.Request {
				return &gemini.Request{
					Prompt:  prompt,
					Model:   cf.model,
					Files:   []string{imgPath},
					JSON:    jsonMode,
					CLIPath: cf.cliPath,
				}
			}
			callGemini := func(r *gemini.Request) (string, error) {
				if cf.payPerUse {
					return gemini.RunREST(r)
				}
				return gemini.Run(r)
			}

			// Step 1: geometry detection (ensemble runs, take median height_m)
			var heightList []float64
			for i := 0; i < ensemble; i++ {
				fmt.Fprintf(os.Stderr, "Step 1: Detecting geometry (run %d/%d)...\n", i+1, ensemble)
				raw, err := callGemini(makeReq(prompts.GeometryPrompt, true))
				if err != nil {
					fmt.Fprintf(os.Stderr, "  geometry error: %v\n", err)
					continue
				}
				var geo struct {
					PlateBox       []float64 `json:"plateBox"`
					TailgateTopY   float64   `json:"tailgateTopY"`
					TailgateBottomY float64  `json:"tailgateBottomY"`
					CargoTopY      float64   `json:"cargoTopY"`
				}
				if err := parseJSONResponse(raw, &geo); err != nil {
					fmt.Fprintf(os.Stderr, "  JSON parse error: %v\n", err)
					continue
				}
				if len(geo.PlateBox) != 4 {
					fmt.Fprintln(os.Stderr, "  plate not detected, skip")
					continue
				}
				tgTop := geo.TailgateTopY
				tgBot := geo.TailgateBottomY
				cargoTop := geo.CargoTopY
				fmt.Fprintf(os.Stderr, "  plate=%v tg_top=%.3f tg_bot=%.3f cargo_top=%.3f\n",
					geo.PlateBox, tgTop, tgBot, cargoTop)
				if tgTop <= 0 || tgBot <= 0 || tgTop >= tgBot {
					fmt.Fprintln(os.Stderr, "  WARNING: tailgate detection invalid, skip")
					continue
				}
				tgHeightNorm := tgBot - tgTop
				mPerNorm := spec.bedHeight / tgHeightNorm
				cargoHeightNorm := tgBot - cargoTop
				cargoHeightM := clamp(cargoHeightNorm*mPerNorm, 0, 0.8)
				fmt.Fprintf(os.Stderr, "  tg_h_norm=%.4f m/norm=%.3f cargo_h=%.3fm\n",
					tgHeightNorm, mPerNorm, cargoHeightM)
				heightList = append(heightList, cargoHeightM)
			}
			if len(heightList) == 0 {
				return fmt.Errorf("geometry detection failed on all runs")
			}
			sort.Float64s(heightList)
			heightM := heightList[len(heightList)/2]
			fmt.Fprintf(os.Stderr, "Height estimates: %v -> median=%.3fm\n", round3Slice(heightList), heightM)

			// Step 2: fill ratio estimation (ensemble runs, average)
			var fillLList, fillWList, packingList []float64
			var lastReasoning string
			for i := 0; i < ensemble; i++ {
				fmt.Fprintf(os.Stderr, "Step 2: Estimating fill (run %d/%d)...\n", i+1, ensemble)
				raw, err := callGemini(makeReq(prompts.FillPrompt, true))
				if err != nil {
					fmt.Fprintf(os.Stderr, "  fill error: %v\n", err)
					continue
				}
				var fill struct {
					FillRatioL    float64 `json:"fillRatioL"`
					FillRatioW    float64 `json:"fillRatioW"`
					PackingDensity float64 `json:"packingDensity"`
					Reasoning     string  `json:"reasoning"`
				}
				if err := parseJSONResponse(raw, &fill); err != nil {
					fmt.Fprintf(os.Stderr, "  JSON parse error: %v\n", err)
					continue
				}
				fl := fill.FillRatioL
				if fl == 0 {
					fl = 0.7
				}
				fw := fill.FillRatioW
				if fw == 0 {
					fw = 0.8
				}
				pd := fill.PackingDensity
				if pd == 0 {
					pd = 0.7
				}
				fmt.Fprintf(os.Stderr, "  L=%v W=%v p=%v\n", fl, fw, pd)
				fillLList = append(fillLList, fl)
				fillWList = append(fillWList, fw)
				packingList = append(packingList, pd)
				if fill.Reasoning != "" {
					lastReasoning = fill.Reasoning
				}
			}
			if len(fillLList) == 0 {
				return fmt.Errorf("fill estimation failed on all runs")
			}
			fillL := clamp(mean(fillLList), 0, 1)
			fillW := clamp(mean(fillWList), 0, 1)
			packing := clamp(mean(packingList), 0.5, 0.9)

			// Step 3: calculate tonnage
			vol := spec.bedLength * spec.bedWidth * heightM * fillL * fillW * shapeFactor
			tonnage := vol * density * packing

			result := EstimateResult{
				Method:            "box-overlay",
				TruckClass:        truckClass,
				MaterialType:      material,
				HeightM:           round3(heightM),
				FillRatioL:        round3(fillL),
				FillRatioW:        round3(fillW),
				PackingDensity:    round3(packing),
				EstimatedVolumeM3: round4(vol),
				EstimatedTonnage:  round2(tonnage),
				Density:           density,
				Reasoning:         lastReasoning,
			}
			b, err := json.MarshalIndent(result, "", "  ")
			if err != nil {
				return fmt.Errorf("marshal result: %w", err)
			}
			fmt.Println(string(b))
			return nil
		},
	}

	c.Flags().StringVarP(&truckClass, "truck-class", "t", "4t", "Truck class (2t, 4t, 増トン, 10t)")
	c.Flags().StringVarP(&material, "material", "M", "As殻", "Material type (土砂, As殻, Co殻, 開粒度As殻)")
	c.Flags().IntVarP(&ensemble, "ensemble", "n", 2, "Number of ensemble runs")
	addCommonFlags(c, &cf)
	return c
}

// parseJSONResponse strips markdown code fences and parses JSON.
func parseJSONResponse(raw string, v any) error {
	// Find first { and last }
	start := -1
	end := -1
	for i, ch := range raw {
		if ch == '{' && start == -1 {
			start = i
		}
		if ch == '}' {
			end = i
		}
	}
	if start != -1 && end != -1 && end > start {
		raw = raw[start : end+1]
	}
	return json.Unmarshal([]byte(raw), v)
}

func clamp(v, lo, hi float64) float64 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func mean(vs []float64) float64 {
	if len(vs) == 0 {
		return 0
	}
	sum := 0.0
	for _, v := range vs {
		sum += v
	}
	return sum / float64(len(vs))
}

func round2(v float64) float64 { return math.Round(v*100) / 100 }
func round3(v float64) float64 { return math.Round(v*1000) / 1000 }
func round4(v float64) float64 { return math.Round(v*10000) / 10000 }

func round3Slice(vs []float64) []string {
	out := make([]string, len(vs))
	for i, v := range vs {
		out[i] = fmt.Sprintf("%.3f", v)
	}
	return out
}
