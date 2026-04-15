//! Backend selection for AI CLI tools

use serde::{Deserialize, Serialize};

/// AI呼び出しの課金モデル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageMode {
    /// 従量課金（APIキー認証、リクエスト毎に課金）
    PayPerUse,
    /// 時間ベース使用量制限（OAuth認証、期間内の無料枠）
    #[default]
    TimeBasedQuota,
    /// 常駐エージェント（deckpilot経由）
    Resident,
}

impl std::fmt::Display for UsageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageMode::PayPerUse => write!(f, "従量課金 (PayPerUse)"),
            UsageMode::TimeBasedQuota => write!(f, "時間ベース使用量制限 (TimeBasedQuota)"),
            UsageMode::Resident => write!(f, "常駐エージェント (Resident)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_mode_default() {
        let mode = UsageMode::default();
        assert_eq!(mode, UsageMode::TimeBasedQuota);
    }

    #[test]
    fn test_usage_mode_display() {
        assert_eq!(format!("{}", UsageMode::PayPerUse), "従量課金 (PayPerUse)");
        assert_eq!(
            format!("{}", UsageMode::TimeBasedQuota),
            "時間ベース使用量制限 (TimeBasedQuota)"
        );
    }

    #[test]
    fn test_usage_mode_equality() {
        assert_eq!(UsageMode::PayPerUse, UsageMode::PayPerUse);
        assert_eq!(UsageMode::TimeBasedQuota, UsageMode::TimeBasedQuota);
        assert_ne!(UsageMode::PayPerUse, UsageMode::TimeBasedQuota);
    }
}
