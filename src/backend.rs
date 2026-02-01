//! Backend selection for AI CLI tools

/// Supported AI backends
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// Google Gemini CLI (default)
    #[default]
    Gemini,
    /// Anthropic Claude CLI
    Claude,
    /// OpenAI Codex CLI
    Codex,
    /// Ollama (future use)
    Ollama,
}

impl Backend {
    /// Get the default command name for this backend
    pub fn command(&self) -> &str {
        match self {
            Backend::Gemini => "gemini",
            Backend::Claude => "claude",
            Backend::Codex => "codex",
            Backend::Ollama => "ollama",
        }
    }

    /// Get the Windows command extension
    pub fn windows_command(&self) -> &str {
        match self {
            Backend::Gemini => "gemini.cmd",
            Backend::Claude => "claude.cmd",
            Backend::Codex => "codex.cmd",
            Backend::Ollama => "ollama.exe",
        }
    }

    /// Get the default model for this backend
    pub fn default_model(&self) -> &str {
        match self {
            Backend::Gemini => "gemini-2.5-flash",
            Backend::Claude => "claude-sonnet-4-20250514",
            Backend::Codex => "gpt-4.1",
            Backend::Ollama => "llama3",
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backend::Gemini => write!(f, "Gemini"),
            Backend::Claude => write!(f, "Claude"),
            Backend::Codex => write!(f, "Codex"),
            Backend::Ollama => write!(f, "Ollama"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_default() {
        let backend = Backend::default();
        assert_eq!(backend, Backend::Gemini);
    }

    #[test]
    fn test_backend_command() {
        assert_eq!(Backend::Gemini.command(), "gemini");
        assert_eq!(Backend::Claude.command(), "claude");
        assert_eq!(Backend::Codex.command(), "codex");
        assert_eq!(Backend::Ollama.command(), "ollama");
    }

    #[test]
    fn test_backend_windows_command() {
        assert_eq!(Backend::Gemini.windows_command(), "gemini.cmd");
        assert_eq!(Backend::Claude.windows_command(), "claude.cmd");
        assert_eq!(Backend::Codex.windows_command(), "codex.cmd");
        assert_eq!(Backend::Ollama.windows_command(), "ollama.exe");
    }

    #[test]
    fn test_backend_default_model() {
        assert_eq!(Backend::Gemini.default_model(), "gemini-2.5-flash");
        assert_eq!(Backend::Claude.default_model(), "claude-sonnet-4-20250514");
        assert_eq!(Backend::Codex.default_model(), "gpt-4.1");
        assert_eq!(Backend::Ollama.default_model(), "llama3");
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(format!("{}", Backend::Gemini), "Gemini");
        assert_eq!(format!("{}", Backend::Claude), "Claude");
        assert_eq!(format!("{}", Backend::Codex), "Codex");
        assert_eq!(format!("{}", Backend::Ollama), "Ollama");
    }
}
