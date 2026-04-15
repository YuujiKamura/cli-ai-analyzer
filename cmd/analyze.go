package cmd

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/spf13/cobra"
)

// NewAnalyzeCmd returns the 'analyze' subcommand.
func NewAnalyzeCmd() *cobra.Command {
	var prompt string
	var cf commonFlags

	c := &cobra.Command{
		Use:     "analyze [flags] <files...>",
		Aliases: []string{"a"},
		Short:   "Analyze files (PDF/images) with a prompt",
		Args:    cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			out, err := runWithFlags(&gemini.Request{
				Prompt:  prompt,
				Model:   cf.model,
				Files:   args,
				JSON:    cf.jsonOut,
				CLIPath: cf.cliPath,
			}, cf.payPerUse)
			if err != nil {
				return err
			}
			fmt.Fprintln(os.Stdout, out)
			return nil
		},
	}

	c.Flags().StringVarP(&prompt, "prompt", "p", "", "Prompt to send to Gemini (required)")
	if err := c.MarkFlagRequired("prompt"); err != nil {
		panic(err)
	}
	addCommonFlags(c, &cf)
	return c
}
