package gemini

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"mime"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// RunREST calls the Gemini REST API (generateContent) directly using GEMINI_API_KEY.
// Used when --pay-per-use is set.
func RunREST(r *Request) (string, error) {
	apiKey := os.Getenv("GEMINI_API_KEY")
	if apiKey == "" {
		return "", fmt.Errorf("GEMINI_API_KEY environment variable is not set. Required for --pay-per-use mode.")
	}

	model := resolvedModel(r)
	if model == "" {
		model = "gemini-2.0-flash-exp"
	}
	url := fmt.Sprintf(
		"https://generativelanguage.googleapis.com/v1beta/models/%s:generateContent?key=%s",
		model, apiKey,
	)

	promptText := r.Prompt
	if r.JSON {
		promptText += " Respond with ONLY the JSON object."
	}

	// Build parts
	parts := []map[string]any{
		{"text": promptText},
	}

	for _, f := range r.Files {
		data, err := os.ReadFile(f)
		if err != nil {
			return "", fmt.Errorf("read file %s: %w", f, err)
		}
		b64 := base64.StdEncoding.EncodeToString(data)
		mimeType := mimeFromPath(f)
		parts = append(parts, map[string]any{
			"inline_data": map[string]any{
				"mime_type": mimeType,
				"data":      b64,
			},
		})
	}

	body := map[string]any{
		"contents": []map[string]any{
			{"parts": parts},
		},
	}
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return "", fmt.Errorf("marshal request: %w", err)
	}

	client := &http.Client{Timeout: 120 * time.Second}
	resp, err := client.Post(url, "application/json", bytes.NewReader(bodyBytes))
	if err != nil {
		return "", fmt.Errorf("HTTP request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("read response body: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		snippet := string(respBody)
		if len(snippet) > 500 {
			snippet = snippet[:500]
		}
		if resp.StatusCode == 401 || resp.StatusCode == 403 {
			return "", fmt.Errorf("gemini API authentication failed (%d): %s", resp.StatusCode, snippet)
		}
		return "", fmt.Errorf("gemini API error (%d): %s", resp.StatusCode, snippet)
	}

	var parsed struct {
		Candidates []struct {
			Content struct {
				Parts []struct {
					Text string `json:"text"`
				} `json:"parts"`
			} `json:"content"`
		} `json:"candidates"`
		Error *struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(respBody, &parsed); err != nil {
		return "", fmt.Errorf("parse API response: %w", err)
	}
	if parsed.Error != nil {
		return "", fmt.Errorf("gemini API error: %s", parsed.Error.Message)
	}
	if len(parsed.Candidates) == 0 || len(parsed.Candidates[0].Content.Parts) == 0 {
		return "", fmt.Errorf("unexpected empty response from Gemini API")
	}

	return strings.TrimSpace(parsed.Candidates[0].Content.Parts[0].Text), nil
}

// mimeFromPath guesses a MIME type from the file extension.
func mimeFromPath(path string) string {
	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".jpg", ".jpeg":
		return "image/jpeg"
	case ".png":
		return "image/png"
	case ".gif":
		return "image/gif"
	case ".webp":
		return "image/webp"
	case ".pdf":
		return "application/pdf"
	case ".mp4":
		return "video/mp4"
	}
	if t := mime.TypeByExtension(ext); t != "" {
		return t
	}
	return "application/octet-stream"
}
