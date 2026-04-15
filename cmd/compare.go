package cmd

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/YuujiKamura/cli-ai-analyzer/internal/prompts"
	"github.com/spf13/cobra"
)

// NewCompareCmd returns the 'compare' subcommand.
func NewCompareCmd() *cobra.Command {
	var instruction string
	var cf commonFlags

	c := &cobra.Command{
		Use:     "compare [flags] <files...>",
		Aliases: []string{"cmp"},
		Short:   "Cross-check multiple files for consistency",
		Args:    cobra.MinimumNArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			prompt := prompts.BuildComparePrompt(args, instruction)
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

	c.Flags().StringVarP(&instruction, "instruction", "i", "", "Custom comparison instructions (appended to default prompt)")
	addCommonFlags(c, &cf)
	return c
}
