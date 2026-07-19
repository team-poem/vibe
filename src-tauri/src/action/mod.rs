mod execute;
mod runner;

pub use execute::{routine_already_assembled, run_routine};
pub use runner::{run, ActionResult, RunError};

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::layout::Region;

/// Accepts both the legacy numeric display id and the current UUID
/// string, so pre-migration routine files never fail to load.
fn de_display<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Num(u64),
        Str(String),
    }
    Ok(
        Option::<Raw>::deserialize(deserializer)?.map(|raw| match raw {
            Raw::Num(id) => id.to_string(),
            Raw::Str(uuid) => uuid,
        }),
    )
}

/// A single macOS action a routine can execute, optionally snapped to a
/// screen region after it opens. MVP supports launching an app and opening
/// a URL; further kinds live in the `poc/action-runner` branch until they
/// are promoted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    OpenApp {
        name: String,
        /// File or folder the app opens on launch (`open -a <name> <path>`),
        /// e.g. an IDE opening a specific project folder.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
        /// Display the region maps onto, as a stable display UUID;
        /// `None` = main display. Legacy files stored numeric CGDisplay
        /// ids, which drift across reboots — they are accepted on load
        /// and migrated by the store.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "de_display"
        )]
        display: Option<String>,
    },
    OpenUrl {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "de_display"
        )]
        display: Option<String>,
    },
    OpenFile {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<Region>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "de_display"
        )]
        display: Option<String>,
    },
}

impl Action {
    pub fn open_app(name: impl Into<String>) -> Self {
        Self::OpenApp {
            name: name.into(),
            path: None,
            region: None,
            display: None,
        }
    }

    pub fn open_url(url: impl Into<String>) -> Self {
        Self::OpenUrl {
            url: url.into(),
            region: None,
            display: None,
        }
    }

    pub fn open_file(path: impl Into<String>) -> Self {
        Self::OpenFile {
            path: path.into(),
            region: None,
            display: None,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } => "open-app",
            Self::OpenUrl { .. } => "open-url",
            Self::OpenFile { .. } => "open-file",
        }
    }

    pub fn region(&self) -> Option<Region> {
        match self {
            Self::OpenApp { region, .. }
            | Self::OpenUrl { region, .. }
            | Self::OpenFile { region, .. } => *region,
        }
    }

    pub fn display(&self) -> Option<&str> {
        match self {
            Self::OpenApp { display, .. }
            | Self::OpenUrl { display, .. }
            | Self::OpenFile { display, .. } => display.as_deref(),
        }
    }

    pub fn display_mut(&mut self) -> &mut Option<String> {
        match self {
            Self::OpenApp { display, .. }
            | Self::OpenUrl { display, .. }
            | Self::OpenFile { display, .. } => display,
        }
    }

    pub(crate) fn program(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } | Self::OpenUrl { .. } | Self::OpenFile { .. } => "open",
        }
    }

    pub(crate) fn args(&self) -> Vec<&str> {
        match self {
            Self::OpenApp {
                name,
                path: Some(path),
                ..
            } => vec!["-a", name.as_str(), path.as_str()],
            Self::OpenApp { name, .. } => vec!["-a", name.as_str()],
            Self::OpenUrl { url, .. } => vec![url.as_str()],
            Self::OpenFile { path, .. } => vec![path.as_str()],
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenApp {
                name,
                path: Some(path),
                ..
            } => {
                let target = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_else(|| path.as_str().into());
                write!(f, "open-app({name}: {target})")
            }
            Self::OpenApp { name, .. } => write!(f, "open-app({name})"),
            Self::OpenUrl { url, .. } => write!(f, "open-url({url})"),
            Self::OpenFile { path, .. } => {
                let file_name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_else(|| path.as_str().into());
                write!(f, "open-file({file_name})")
            }
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
    fn open_app_with_path_appends_the_target() {
        let action = Action::OpenApp {
            name: "Cursor".to_owned(),
            path: Some("/Users/me/projects/vibe".to_owned()),
            region: None,
            display: None,
        };
        assert_eq!(
            action.args(),
            vec!["-a", "Cursor", "/Users/me/projects/vibe"]
        );
        assert_eq!(action.to_string(), "open-app(Cursor: vibe)");
    }

    #[test]
    fn open_app_without_path_serializes_without_the_field() {
        let json = serde_json::to_string(&Action::open_app("Cursor")).expect("serialize");
        assert_eq!(json, r#"{"type":"open-app","name":"Cursor"}"#);
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
