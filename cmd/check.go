package cmd

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/YuujiKamura/cli-ai-analyzer/internal/prompts"
	"github.com/spf13/cobra"
)

// NewCheckCmd returns the 'check' subcommand.
func NewCheckCmd() *cobra.Command {
	var instruction string
	var cf commonFlags

	c := &cobra.Command{
		Use:     "check [flags] <files...>",
		Aliases: []string{"c"},
		Short:   "Japanese document verification check",
		Args:    cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			prompt := prompts.BuildCheckPrompt(instruction)
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

	c.Flags().StringVarP(&instruction, "instruction", "i", "", "Custom verification instructions (appended to default prompt)")
	addCommonFlags(c, &cf)
	return c
}
