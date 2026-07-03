use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;

const LOG_CAPACITY: usize = 50;

/// Result of one action within a routine run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub label: String,
    pub success: bool,
    pub detail: String,
}

/// One routine execution, newest kept first in the log.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecord {
    pub at_epoch_ms: u64,
    pub routine_name: String,
    pub success: bool,
    pub outcomes: Vec<ActionOutcome>,
}

/// In-memory ring buffer of recent routine runs (PRD 7.6). Not persisted:
/// the log is diagnostic, not user data.
pub struct ExecutionLog {
    records: Mutex<VecDeque<ExecutionRecord>>,
}

impl ExecutionLog {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(VecDeque::with_capacity(LOG_CAPACITY)),
        }
    }

    pub fn push(&self, record: ExecutionRecord) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if records.len() == LOG_CAPACITY {
            records.pop_back();
        }
        records.push_front(record);
    }

    /// Newest first.
    pub fn snapshot(&self) -> Vec<ExecutionRecord> {
        self.records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}

impl Default for ExecutionLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str) -> ExecutionRecord {
        ExecutionRecord {
            at_epoch_ms: 0,
            routine_name: name.to_owned(),
            success: true,
            outcomes: Vec::new(),
        }
    }

    #[test]
    fn snapshot_returns_newest_first() {
        let log = ExecutionLog::new();
        log.push(record("first"));
        log.push(record("second"));
        let names: Vec<String> = log.snapshot().into_iter().map(|r| r.routine_name).collect();
        assert_eq!(names, vec!["second", "first"]);
    }

    #[test]
    fn capacity_drops_oldest() {
        let log = ExecutionLog::new();
        for i in 0..(LOG_CAPACITY + 5) {
            log.push(record(&format!("run-{i}")));
        }
        let snapshot = log.snapshot();
        assert_eq!(snapshot.len(), LOG_CAPACITY);
        assert_eq!(
            snapshot[0].routine_name,
            format!("run-{}", LOG_CAPACITY + 4)
        );
    }
}
