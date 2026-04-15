// Package gemini provides Gemini CLI subprocess invocation and REST API access.
package gemini

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

// Request holds the parameters for a Gemini call.
type Request struct {
	Prompt  string
	Model   string
	Files   []string // absolute paths
	JSON    bool
	CLIPath string // custom override; empty = auto-resolve
}

func resolvedModel(r *Request) string {
	return r.Model
}

// jsonSuffix returns the prompt suffix to request JSON output.
// The Rust executor always used text output mode for the CLI and appended
// " Respond with ONLY the JSON object." to the prompt for JSON requests.
func jsonSuffix(r *Request) string {
	if r.JSON {
		return " Respond with ONLY the JSON object."
	}
	return ""
}

// Run invokes the Gemini CLI subprocess and returns its combined output.
// File paths are converted to neutral copies (image_0.ext …) in a temp dir
// so that filenames do not leak to the model — matching the Rust executor behaviour.
func Run(r *Request) (string, error) {
	cliPath, err := ResolveCLIPath(r.CLIPath)
	if err != nil {
		return "", err
	}

	if runtime.GOOS == "windows" {
		return runWindows(cliPath, r)
	}
	return runUnix(cliPath, r)
}

// runWindows builds and executes a PowerShell script, mirroring build_ps_script.
func runWindows(cliPath string, r *Request) (string, error) {
	script, tmpDir, err := buildPSScript(cliPath, r)
	if err != nil {
		return "", err
	}
	if tmpDir != "" {
		defer os.RemoveAll(tmpDir)
	}

	// Write script to a temp file
	sf, err := os.CreateTemp("", "gemini-*.ps1")
	if err != nil {
		return "", fmt.Errorf("create temp script: %w", err)
	}
	defer os.Remove(sf.Name())
	if _, err := sf.WriteString(script); err != nil {
		sf.Close()
		return "", err
	}
	sf.Close()

	cmd := exec.Command("powershell",
		"-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
		"-File", sf.Name(),
	)
	if tmpDir != "" {
		cmd.Dir = tmpDir
	}
	return execCmd(cmd)
}

// runUnix builds and executes a bash script.
func runUnix(cliPath string, r *Request) (string, error) {
	script, tmpDir, err := buildShellScript(cliPath, r)
	if err != nil {
		return "", err
	}
	if tmpDir != "" {
		defer os.RemoveAll(tmpDir)
	}

	sf, err := os.CreateTemp("", "gemini-*.sh")
	if err != nil {
		return "", fmt.Errorf("create temp script: %w", err)
	}
	defer os.Remove(sf.Name())
	if _, err := sf.WriteString(script); err != nil {
		sf.Close()
		return "", err
	}
	sf.Close()
	os.Chmod(sf.Name(), 0o755)

	cmd := exec.Command("bash", sf.Name())
	if tmpDir != "" {
		cmd.Dir = tmpDir
	}
	return execCmd(cmd)
}

// buildPSScript returns the PowerShell script body and the temp dir (if any).
// Mirrors build_ps_script from the Rust executor.
func buildPSScript(cliPath string, r *Request) (script, tmpDir string, err error) {
	model := resolvedModel(r)
	suffix := jsonSuffix(r)
	pathEsc := strings.ReplaceAll(cliPath, "'", "''")

	modelOpt := ""
	if model != "" {
		modelOpt = "-m " + model + " "
	}

	if len(r.Files) > 0 {
		tmpDir, err = os.MkdirTemp("", "gemini-files-*")
		if err != nil {
			return "", "", fmt.Errorf("create temp dir: %w", err)
		}

		var copyLines strings.Builder
		var refs []string
		for i, f := range r.Files {
			ext := filepath.Ext(f)
			if ext == "" {
				ext = ".jpg"
			}
			neutral := fmt.Sprintf("image_%d%s", i, ext)
			neutralPath := filepath.Join(tmpDir, neutral)
			if err := copyFile(f, neutralPath); err != nil {
				os.RemoveAll(tmpDir)
				return "", "", fmt.Errorf("copy file %s: %w", f, err)
			}
			copyLines.WriteString(fmt.Sprintf("# file already copied: %s\n", neutral))
			refs = append(refs, "@"+neutral)
		}

		fullPrompt := strings.Join(refs, " ") + " " + r.Prompt + suffix
		promptEsc := strings.ReplaceAll(fullPrompt, "'", "''")

		script = fmt.Sprintf(
			"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8\n"+
				"%s'%s' | & '%s' %s-o text\n",
			copyLines.String(),
			promptEsc,
			pathEsc,
			modelOpt,
		)
	} else {
		// No files: use -p flag directly (works on gemini CLI >= v0.27.1)
		promptEsc := strings.ReplaceAll(r.Prompt+suffix, "'", "''")
		script = fmt.Sprintf(
			"$OutputEncoding = [Console]::OutputEncoding = [Text.Encoding]::UTF8\n"+
				"& '%s' %s-p '%s' -o text\n",
			pathEsc,
			modelOpt,
			promptEsc,
		)
	}
	return script, tmpDir, nil
}

// buildShellScript returns the bash script body and the temp dir (if any).
// Mirrors build_shell_script from the Rust executor.
func buildShellScript(cliPath string, r *Request) (script, tmpDir string, err error) {
	model := resolvedModel(r)
	suffix := jsonSuffix(r)
	pathEsc := strings.ReplaceAll(cliPath, "'", "'\\''")

	modelOpt := ""
	if model != "" {
		modelOpt = "-m " + model + " "
	}

	if len(r.Files) > 0 {
		tmpDir, err = os.MkdirTemp("", "gemini-files-*")
		if err != nil {
			return "", "", fmt.Errorf("create temp dir: %w", err)
		}

		var copyLines strings.Builder
		var refs []string
		for i, f := range r.Files {
			ext := filepath.Ext(f)
			if ext == "" {
				ext = ".jpg"
			}
			neutral := fmt.Sprintf("image_%d%s", i, ext)
			neutralPath := filepath.Join(tmpDir, neutral)
			if err := copyFile(f, neutralPath); err != nil {
				os.RemoveAll(tmpDir)
				return "", "", fmt.Errorf("copy file %s: %w", f, err)
			}
			srcEsc := strings.ReplaceAll(f, "'", "'\\''")
			copyLines.WriteString(fmt.Sprintf("cp '%s' '%s'\n", srcEsc, neutralPath))
			refs = append(refs, "@"+neutral)
		}

		fullPrompt := strings.Join(refs, " ") + " " + r.Prompt + suffix
		promptEsc := strings.ReplaceAll(fullPrompt, "'", "'\\''")

		script = fmt.Sprintf(
			"#!/bin/bash\n%secho '%s' | '%s' %s-o text\n",
			copyLines.String(),
			promptEsc,
			pathEsc,
			modelOpt,
		)
	} else {
		promptEsc := strings.ReplaceAll(r.Prompt+suffix, "'", "'\\''")
		script = fmt.Sprintf(
			"#!/bin/bash\n'%s' %s-p '%s' -o text\n",
			pathEsc,
			modelOpt,
			promptEsc,
		)
	}
	return script, tmpDir, nil
}

// execCmd runs a command, returns trimmed stdout or error.
func execCmd(cmd *exec.Cmd) (string, error) {
	cmd.Stdin = nil
	out, err := cmd.Output()
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			stderr := strings.TrimSpace(string(exitErr.Stderr))
			stdout := strings.TrimSpace(string(out))
			detail := stderr
			if stdout != "" {
				detail = stderr + "\n" + stdout
			}
			detail = strings.TrimSpace(detail)
			if strings.Contains(detail, "not found") || strings.Contains(detail, "not recognized") {
				return "", fmt.Errorf("gemini CLI not found. Install from https://github.com/google-gemini/gemini-cli or set GEMINI_CMD_PATH")
			}
			if strings.Contains(detail, "authenticate") || strings.Contains(detail, "auth") {
				return "", fmt.Errorf("gemini authentication failed: %s", detail)
			}
			return "", fmt.Errorf("gemini CLI error: %s", detail)
		}
		return "", fmt.Errorf("exec error: %w", err)
	}
	result := strings.TrimSpace(string(out))
	return result, nil
}

// copyFile copies src to dst.
func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, in)
	return err
}
