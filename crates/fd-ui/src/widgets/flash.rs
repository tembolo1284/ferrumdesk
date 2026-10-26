use egui::Color32;
use fd_types::QuoteId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks the last seen value per quote so cells can flash on change.
#[derive(Default)]
pub struct Flash {
    last: HashMap<QuoteId, (f64, Instant, bool)>, // (value, when changed, up?)
    hold: Duration,
}

impl Flash {
    pub fn new(hold_ms: u64) -> Self { Self { last: HashMap::new(), hold: Duration::from_millis(hold_ms) } }

    /// Call once per frame per quote; returns the tint to apply, if any.
    pub fn observe(&mut self, id: &QuoteId, value: f64) -> Option<Color32> {
        let now = Instant::now();
        let entry = self.last.entry(id.clone()).or_insert((value, now - self.hold, true));
        if (value - entry.0).abs() > f64::EPSILON {
            entry.2 = value > entry.0;
            entry.0 = value;
            entry.1 = now;
        }
        let age = now.duration_since(entry.1);
        if age < self.hold {
            let a = (1.0 - age.as_secs_f32() / self.hold.as_secs_f32()) * 90.0;
            let base = if entry.2 { crate::theme::UP } else { crate::theme::DOWN };
            Some(Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a as u8))
        } else {
            None
        }
    }
}
