package cmd

import (
	"fmt"
	"os"

	"github.com/YuujiKamura/cli-ai-analyzer/internal/gemini"
	"github.com/spf13/cobra"
)

// NewPromptCmd returns the 'prompt' subcommand.
func NewPromptCmd() *cobra.Command {
	var cf commonFlags

	c := &cobra.Command{
		Use:     "prompt <PROMPT>",
		Aliases: []string{"p"},
		Short:   "Send a text-only prompt (no files)",
		Args:    cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			out, err := runWithFlags(&gemini.Request{
				Prompt:  args[0],
				Model:   cf.model,
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

	addCommonFlags(c, &cf)
	return c
}
