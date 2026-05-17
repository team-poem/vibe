use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClapEvent {
    pub timestamp_ms: u64,
    pub peak_db: f32,
    pub above_floor_db: f32,
    pub flatness: f32,
    pub confidence: f32,
}
