mod execute;
mod runner;

pub use execute::run_routine;
pub use runner::{run, ActionResult, RunError};

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::layout::Region;

/// A single macOS action a routine can execute, optionally snapped to a
/// screen region after it opens. MVP supports launching an app and opening
/// a URL; further kinds live in the `poc/action-runner` branch until they
/// are promoted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    OpenApp {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
    },
    OpenUrl {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
    },
}

impl Action {
    pub fn open_app(name: impl Into<String>) -> Self {
        Self::OpenApp {
            name: name.into(),
            region: None,
        }
    }

    pub fn open_url(url: impl Into<String>) -> Self {
        Self::OpenUrl {
            url: url.into(),
            region: None,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } => "open-app",
            Self::OpenUrl { .. } => "open-url",
        }
    }

    pub fn region(&self) -> Option<Region> {
        match self {
            Self::OpenApp { region, .. } | Self::OpenUrl { region, .. } => *region,
        }
    }

    pub(crate) fn program(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } | Self::OpenUrl { .. } => "open",
        }
    }

    pub(crate) fn args(&self) -> Vec<&str> {
        match self {
            Self::OpenApp { name, .. } => vec!["-a", name.as_str()],
            Self::OpenUrl { url, .. } => vec![url.as_str()],
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenApp { name, .. } => write!(f, "open-app({name})"),
            Self::OpenUrl { url, .. } => write!(f, "open-url({url})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_app_produces_open_dash_a_args() {
        let action = Action::open_app("Calculator");
        assert_eq!(action.program(), "open");
        assert_eq!(action.args(), vec!["-a", "Calculator"]);
        assert_eq!(action.kind_label(), "open-app");
    }

    #[test]
    fn open_url_passes_url_directly_to_open() {
        let action = Action::open_url("https://example.com");
        assert_eq!(action.program(), "open");
        assert_eq!(action.args(), vec!["https://example.com"]);
        assert_eq!(action.kind_label(), "open-url");
    }

    #[test]
    fn serializes_with_kind_tag() {
        let json = serde_json::to_string(&Action::open_app("Cursor")).expect("serialize");
        assert_eq!(json, r#"{"type":"open-app","name":"Cursor"}"#);
    }

    #[test]
    fn deserializes_from_kind_tag() {
        let action: Action =
            serde_json::from_str(r#"{"type":"open-url","url":"https://github.com"}"#)
                .expect("deserialize");
        assert_eq!(action, Action::open_url("https://github.com"));
    }

    #[test]
    fn rejects_unknown_kind() {
        let result = serde_json::from_str::<Action>(r#"{"type":"osascript","script":"x"}"#);
        assert!(result.is_err());
    }
}
