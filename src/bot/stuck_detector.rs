use std::time::{Duration, Instant};

use crate::bot::vec2::Vec2f;

const MIN_DURATION: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct StuckDetector {
    last_position: Option<Vec2f>,
    last_update: Option<Instant>,
}

impl StuckDetector {
    pub fn new() -> Self {
        Self {
            last_position: None,
            last_update: None,
        }
    }

    pub fn update(&mut self, position: Vec2f, now: Instant) {
        self.last_position = Some(position);
        self.last_update = Some(now);
    }

    pub fn check(&self, position: Vec2f, now: Instant) -> bool {
        self.last_update
            .map(|update| self.last_position == Some(position) && now - update > MIN_DURATION)
            .unwrap_or(false)
    }
}
