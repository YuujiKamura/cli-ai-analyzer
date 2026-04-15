// Package cmd contains cobra subcommand implementations.
package cmd

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/spf13/cobra"
)

// DefaultModel is the default Gemini model.
// flash-preview is fast and cheap; use `-m gemini-3-pro-preview` when needing
// higher fidelity on structured output prompts (pro avoids the hallucinated
// fill-in-the-blank output mode that flash can fall into on verbose templates).
const DefaultModel = "gemini-3-flash-preview"

// commonFlags holds flags shared across all subcommands.
type commonFlags struct {
	model     string
	jsonOut   bool
	cliPath   string
	payPerUse bool
}

// addCommonFlags binds common flags to a cobra command.
func addCommonFlags(c *cobra.Command, f *commonFlags) {
	c.Flags().StringVarP(&f.model, "model", "m", DefaultModel, "Gemini model")
	c.Flags().BoolVar(&f.jsonOut, "json", false, "Request JSON output")
	c.Flags().StringVar(&f.cliPath, "cli-path", "", "Custom gemini CLI path (overrides GEMINI_CMD_PATH)")
	c.Flags().BoolVar(&f.payPerUse, "pay-per-use", false, "Use Gemini REST API via GEMINI_API_KEY instead of CLI subprocess")
}

// runWithFlags validates files, then dispatches to REST API or CLI subprocess.
func runWithFlags(r *gemini.Request, payPerUse bool) (string, error) {
	for _, f := range r.Files {
		if _, err := os.Stat(f); err != nil {
			return "", fmt.Errorf("file not found: %s", f)
		}
	}
	if payPerUse {
		return gemini.RunREST(r)
	}
	return gemini.Run(r)
}
