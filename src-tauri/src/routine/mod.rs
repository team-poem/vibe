mod exec_log;
mod store;

pub use exec_log::{ActionOutcome, ExecutionLog, ExecutionRecord};
pub use store::{LoadReport, RoutineStore, StoreError};

use serde::{Deserialize, Serialize};

use crate::action::Action;

/// A named, ordered list of actions the user triggers with a double clap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub actions: Vec<Action>,
}

/// The persisted routine document: every routine the user owns plus which
/// one currently reacts to the trigger. Exactly zero or one routine is
/// active at a time (PRD 7.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineConfig {
    pub active_routine_id: Option<String>,
    pub routines: Vec<Routine>,
}

impl RoutineConfig {
    /// Initial document for first launch and corruption recovery: one
    /// sample routine, active, so a double clap does something visible
    /// before the user edits anything.
    pub fn default_config() -> Self {
        let sample = Routine {
            id: "sample".to_owned(),
            name: "Sample — Calculator".to_owned(),
            actions: vec![Action::open_app("Calculator")],
        };
        Self {
            active_routine_id: Some(sample.id.clone()),
            routines: vec![sample],
        }
    }

    pub fn active_routine(&self) -> Option<&Routine> {
        let active_id = self.active_routine_id.as_deref()?;
        self.routines.iter().find(|r| r.id == active_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_an_active_sample_routine() {
        let config = RoutineConfig::default_config();
        let active = config.active_routine().expect("sample must be active");
        assert!(!active.actions.is_empty());
    }

    #[test]
    fn active_routine_is_none_when_id_dangles() {
        let mut config = RoutineConfig::default_config();
        config.active_routine_id = Some("missing".to_owned());
        assert!(config.active_routine().is_none());
    }

    #[test]
    fn config_json_roundtrip() {
        let config = RoutineConfig::default_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let back: RoutineConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, back);
    }
}
