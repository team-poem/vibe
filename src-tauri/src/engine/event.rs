/// A single detected clap, summarizing the signal features the matcher needs.
#[derive(Debug, Clone, Copy)]
pub struct ClapEvent {
    /// Milliseconds since detection started (frame-index based).
    pub timestamp_ms: u64,
    pub peak_db: f32,
    pub above_floor_db: f32,
    pub flatness: f32,
    pub confidence: f32,
}
