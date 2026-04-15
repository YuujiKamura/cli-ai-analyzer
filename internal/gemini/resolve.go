package gemini

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
)

// ResolveCLIPath returns the gemini CLI path to use, in priority order:
//  1. custom (from --cli-path flag)
//  2. GEMINI_CMD_PATH env var
//  3. OS default (gemini.cmd on Windows, gemini elsewhere)
func ResolveCLIPath(custom string) (string, error) {
	if custom != "" {
		return custom, nil
	}
	if env := os.Getenv("GEMINI_CMD_PATH"); env != "" {
		return env, nil
	}

	// On Windows try gemini.cmd first, then gemini.exe, then gemini
	if runtime.GOOS == "windows" {
		for _, name := range []string{"gemini.cmd", "gemini.exe", "gemini"} {
			if p, err := exec.LookPath(name); err == nil {
				return p, nil
			}
		}
	} else {
		if p, err := exec.LookPath("gemini"); err == nil {
			return p, nil
		}
	}

	return "", fmt.Errorf(
		"gemini CLI not found on PATH.\n" +
			"Install it from https://github.com/google-gemini/gemini-cli or set GEMINI_CMD_PATH to its location.",
	)
}
