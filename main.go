package main

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/cmd"
	"github.com/spf13/cobra"
)

func main() {
	root := &cobra.Command{
		Use:   "cli-ai-analyzer",
		Short: "AI analysis CLI powered by Gemini",
		Long: `A thin Gemini CLI wrapper for AI analysis of files and text.

Supports PDF, image, and text file analysis with customizable prompts.
Uses the gemini CLI subprocess or Gemini REST API (--pay-per-use).`,
	}

	root.AddCommand(
		cmd.NewAnalyzeCmd(),
		cmd.NewPromptCmd(),
		cmd.NewCheckCmd(),
		cmd.NewCompareCmd(),
		cmd.NewEstimateCmd(),
	)

	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
