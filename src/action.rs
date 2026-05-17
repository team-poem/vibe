use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    OpenApp { name: String },
    OpenUrl { url: String },
    Osascript { script: String },
    Shortcut { name: String },
}

impl Action {
    pub fn open_app(name: impl Into<String>) -> Self {
        Self::OpenApp { name: name.into() }
    }

    pub fn open_url(url: impl Into<String>) -> Self {
        Self::OpenUrl { url: url.into() }
    }

    pub fn osascript(script: impl Into<String>) -> Self {
        Self::Osascript {
            script: script.into(),
        }
    }

    pub fn shortcut(name: impl Into<String>) -> Self {
        Self::Shortcut { name: name.into() }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } => "open-app",
            Self::OpenUrl { .. } => "open-url",
            Self::Osascript { .. } => "osascript",
            Self::Shortcut { .. } => "shortcut",
        }
    }

    pub fn program(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } | Self::OpenUrl { .. } => "open",
            Self::Osascript { .. } => "osascript",
            Self::Shortcut { .. } => "shortcuts",
        }
    }

    pub fn args(&self) -> Vec<&str> {
        match self {
            Self::OpenApp { name } => vec!["-a", name.as_str()],
            Self::OpenUrl { url } => vec![url.as_str()],
            Self::Osascript { script } => vec!["-e", script.as_str()],
            Self::Shortcut { name } => vec!["run", name.as_str()],
        }
    }

    pub fn parse(kind: &str, param: &str) -> Option<Self> {
        match kind {
            "open-app" => Some(Self::open_app(param)),
            "open-url" => Some(Self::open_url(param)),
            "osascript" => Some(Self::osascript(param)),
            "shortcut" => Some(Self::shortcut(param)),
            _ => None,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenApp { name } => write!(f, "open-app({name})"),
            Self::OpenUrl { url } => write!(f, "open-url({url})"),
            Self::Osascript { .. } => write!(f, "osascript(...)"),
            Self::Shortcut { name } => write!(f, "shortcut({name})"),
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
    }

    #[test]
    fn osascript_uses_dash_e_flag() {
        let action = Action::osascript("return 1");
        assert_eq!(action.program(), "osascript");
        assert_eq!(action.args(), vec!["-e", "return 1"]);
    }

    #[test]
    fn shortcut_uses_run_subcommand() {
        let action = Action::shortcut("test-shortcut");
        assert_eq!(action.program(), "shortcuts");
        assert_eq!(action.args(), vec!["run", "test-shortcut"]);
    }

    #[test]
    fn parse_recognizes_all_known_kinds() {
        assert_eq!(
            Action::parse("open-app", "Calculator"),
            Some(Action::open_app("Calculator"))
        );
        assert_eq!(
            Action::parse("open-url", "https://x"),
            Some(Action::open_url("https://x"))
        );
        assert_eq!(
            Action::parse("osascript", "return 1"),
            Some(Action::osascript("return 1"))
        );
        assert_eq!(
            Action::parse("shortcut", "test"),
            Some(Action::shortcut("test"))
        );
    }

    #[test]
    fn parse_returns_none_for_unknown_kind() {
        assert_eq!(Action::parse("bogus", "x"), None);
    }
}
