use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::action::Action;
use crate::routine::{Routine, RoutineConfig};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to access the routine file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode routines: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("routine not found: {0}")]
    NotFound(String),
}

/// How the persisted document was obtained at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadReport {
    Loaded,
    CreatedDefault,
    RecoveredFromCorruption,
}

/// Thread-safe owner of the routine document. Every mutation persists to
/// disk before returning so the file is always the source of truth.
pub struct RoutineStore {
    path: PathBuf,
    config: Mutex<RoutineConfig>,
}

impl RoutineStore {
    /// Load the document from `path`, falling back to the default config
    /// when the file is missing (first launch) or unreadable (corruption).
    /// A corrupt file is kept next to the original as `<name>.corrupt`
    /// for diagnosis instead of being silently destroyed.
    pub fn load_or_recover(path: PathBuf) -> (Self, LoadReport) {
        let (config, report) = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<RoutineConfig>(&bytes) {
                Ok(config) => (config, LoadReport::Loaded),
                Err(_) => {
                    let _ = std::fs::rename(&path, corrupt_backup_path(&path));
                    (
                        RoutineConfig::default_config(),
                        LoadReport::RecoveredFromCorruption,
                    )
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                (RoutineConfig::default_config(), LoadReport::CreatedDefault)
            }
            Err(_) => (
                RoutineConfig::default_config(),
                LoadReport::RecoveredFromCorruption,
            ),
        };

        let store = Self {
            path,
            config: Mutex::new(config),
        };
        if report != LoadReport::Loaded {
            if let Err(err) = store.save_locked(&store.lock()) {
                eprintln!("[routine] failed to persist recovered config: {err}");
            }
        }
        (store, report)
    }

    pub fn snapshot(&self) -> RoutineConfig {
        self.lock().clone()
    }

    /// Actions of the active routine, cloned out so the caller never holds
    /// the lock while executing them.
    pub fn active_actions(&self) -> Vec<Action> {
        let config = self.lock();
        config
            .active_routine()
            .map(|r| r.actions.clone())
            .unwrap_or_default()
    }

    /// Insert or replace a routine by id. An empty id means "new": the
    /// store assigns one. Returns the updated document.
    pub fn upsert_routine(&self, mut routine: Routine) -> Result<RoutineConfig, StoreError> {
        let mut config = self.lock();
        if routine.id.is_empty() {
            routine.id = uuid::Uuid::new_v4().to_string();
        }
        match config.routines.iter_mut().find(|r| r.id == routine.id) {
            Some(existing) => *existing = routine,
            None => config.routines.push(routine),
        }
        self.save_locked(&config)?;
        Ok(config.clone())
    }

    /// Remove a routine. If it was active, the document ends up with no
    /// active routine rather than a dangling id.
    pub fn delete_routine(&self, id: &str) -> Result<RoutineConfig, StoreError> {
        let mut config = self.lock();
        let before = config.routines.len();
        config.routines.retain(|r| r.id != id);
        if config.routines.len() == before {
            return Err(StoreError::NotFound(id.to_owned()));
        }
        if config.active_routine_id.as_deref() == Some(id) {
            config.active_routine_id = None;
        }
        self.save_locked(&config)?;
        Ok(config.clone())
    }

    /// Switch the active routine. `None` disables triggering entirely.
    pub fn set_active_routine(&self, id: Option<String>) -> Result<RoutineConfig, StoreError> {
        let mut config = self.lock();
        if let Some(id) = &id {
            if !config.routines.iter().any(|r| &r.id == id) {
                return Err(StoreError::NotFound(id.clone()));
            }
        }
        config.active_routine_id = id;
        self.save_locked(&config)?;
        Ok(config.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RoutineConfig> {
        // A poisoned lock means a panic mid-mutation; the in-memory config
        // is still structurally valid, so continue with it.
        self.config
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Persist while holding the config lock. Deliberately violates the
    /// "no lock across I/O" default: the write is a few KB to a local
    /// file, and writing inside the lock is what guarantees the file
    /// never sees out-of-order snapshots from racing mutations.
    fn save_locked(&self, config: &RoutineConfig) -> Result<(), StoreError> {
        let json = serde_json::to_vec_pretty(config)?;
        let tmp_path = self.path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.corrupt")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routine(id: &str, name: &str) -> Routine {
        Routine {
            id: id.to_owned(),
            name: name.to_owned(),
            actions: vec![Action::open_app("Calculator")],
        }
    }

    fn temp_store_path(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vibe-routine-store-{test_name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("routines.json")
    }

    #[test]
    fn first_launch_creates_default_file() {
        let path = temp_store_path("first-launch");
        let (store, report) = RoutineStore::load_or_recover(path.clone());
        assert_eq!(report, LoadReport::CreatedDefault);
        assert!(path.exists(), "default config must be persisted");
        assert!(store.snapshot().active_routine().is_some());
    }

    #[test]
    fn saved_config_survives_reload() {
        let path = temp_store_path("reload");
        let (store, _) = RoutineStore::load_or_recover(path.clone());
        store
            .upsert_routine(routine("dev", "Dev start"))
            .expect("upsert");

        let (reloaded, report) = RoutineStore::load_or_recover(path);
        assert_eq!(report, LoadReport::Loaded);
        assert!(reloaded.snapshot().routines.iter().any(|r| r.id == "dev"));
    }

    #[test]
    fn corrupt_file_recovers_to_default_and_keeps_backup() {
        let path = temp_store_path("corrupt");
        std::fs::write(&path, b"{ not json").expect("write garbage");

        let (store, report) = RoutineStore::load_or_recover(path.clone());
        assert_eq!(report, LoadReport::RecoveredFromCorruption);
        assert!(store.snapshot().active_routine().is_some());
        assert!(corrupt_backup_path(&path).exists(), "backup must be kept");
    }

    #[test]
    fn upsert_with_empty_id_assigns_one() {
        let path = temp_store_path("assign-id");
        let (store, _) = RoutineStore::load_or_recover(path);
        let config = store
            .upsert_routine(routine("", "New routine"))
            .expect("upsert");
        let created = config
            .routines
            .iter()
            .find(|r| r.name == "New routine")
            .expect("created routine");
        assert!(!created.id.is_empty());
    }

    #[test]
    fn upsert_replaces_by_id() {
        let path = temp_store_path("replace");
        let (store, _) = RoutineStore::load_or_recover(path);
        store
            .upsert_routine(routine("dev", "Before"))
            .expect("insert");
        let config = store
            .upsert_routine(routine("dev", "After"))
            .expect("update");
        let updated = config
            .routines
            .iter()
            .find(|r| r.id == "dev")
            .expect("kept");
        assert_eq!(updated.name, "After");
        assert_eq!(config.routines.iter().filter(|r| r.id == "dev").count(), 1);
    }

    #[test]
    fn delete_active_routine_clears_active_id() {
        let path = temp_store_path("delete-active");
        let (store, _) = RoutineStore::load_or_recover(path);
        store.upsert_routine(routine("dev", "Dev")).expect("insert");
        store
            .set_active_routine(Some("dev".to_owned()))
            .expect("activate");

        let config = store.delete_routine("dev").expect("delete");
        assert_eq!(config.active_routine_id, None);
        assert!(store.active_actions().is_empty());
    }

    #[test]
    fn delete_unknown_routine_errors() {
        let path = temp_store_path("delete-unknown");
        let (store, _) = RoutineStore::load_or_recover(path);
        assert!(matches!(
            store.delete_routine("nope"),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn set_active_rejects_unknown_id() {
        let path = temp_store_path("activate-unknown");
        let (store, _) = RoutineStore::load_or_recover(path);
        assert!(matches!(
            store.set_active_routine(Some("nope".to_owned())),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn active_actions_follow_the_active_routine() {
        let path = temp_store_path("active-actions");
        let (store, _) = RoutineStore::load_or_recover(path);
        let mut dev = routine("dev", "Dev");
        dev.actions = vec![Action::open_url("https://github.com")];
        store.upsert_routine(dev).expect("insert");
        store
            .set_active_routine(Some("dev".to_owned()))
            .expect("activate");

        assert_eq!(
            store.active_actions(),
            vec![Action::open_url("https://github.com")]
        );

        store.set_active_routine(None).expect("deactivate");
        assert!(store.active_actions().is_empty());
    }
}
