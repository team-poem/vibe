pub struct AdaptiveFloor {
    db: f32,
    alpha: f32,
}

impl AdaptiveFloor {
    pub fn new(initial_db: f32, alpha: f32) -> Self {
        Self {
            db: initial_db,
            alpha,
        }
    }

    pub fn current_db(&self) -> f32 {
        self.db
    }

    pub fn update(&mut self, frame_db: f32) {
        if !frame_db.is_finite() {
            return;
        }
        self.db = self.db * (1.0 - self.alpha) + frame_db * self.alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_converges_to_steady_input() {
        let mut floor = AdaptiveFloor::new(-60.0, 0.1);
        for _ in 0..200 {
            floor.update(-50.0);
        }
        assert!((floor.current_db() - (-50.0)).abs() < 0.5);
    }

    #[test]
    fn floor_ignores_non_finite_input() {
        let mut floor = AdaptiveFloor::new(-60.0, 0.1);
        floor.update(f32::NEG_INFINITY);
        assert!((floor.current_db() - (-60.0)).abs() < 0.001);
    }
}
