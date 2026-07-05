pub mod detector;
pub mod event;
pub mod features;
pub mod floor;
pub mod matcher;

use serde::{Deserialize, Serialize};

/// How eagerly the engine accepts a double clap. `Low` is the strictest
/// tuning (fewest false triggers), `High` the most permissive (fewest
/// missed claps). Persisted as a user setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    Low,
    #[default]
    Medium,
    High,
}

impl Sensitivity {
    /// Stable numeric encoding for lock-free sharing across threads.
    pub fn as_u8(self) -> u8 {
        match self {
            Sensitivity::Low => 0,
            Sensitivity::Medium => 1,
            Sensitivity::High => 2,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Sensitivity::Low,
            2 => Sensitivity::High,
            _ => Sensitivity::Medium,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_u8_roundtrip() {
        for level in [Sensitivity::Low, Sensitivity::Medium, Sensitivity::High] {
            assert_eq!(Sensitivity::from_u8(level.as_u8()), level);
        }
    }

    #[test]
    fn sensitivity_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Sensitivity::Medium).unwrap(),
            "\"medium\""
        );
    }
}
