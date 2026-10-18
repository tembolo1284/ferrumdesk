use crate::snapshot::QuoteBook;
use fd_types::{Quote, QuoteId, Timestamp};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Minimum interval between emitted snapshots.
    pub min_interval: Duration,
    /// Drop quotes older than this on arrival (bad replay clocks, stale feeds).
    pub max_quote_age: chrono::Duration,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self { min_interval: Duration::from_millis(250), max_quote_age: chrono::Duration::seconds(30) }
    }
}

/// Coalesces a tick stream into "at most one snapshot per `min_interval`,
/// containing the latest value of every quote that moved". Owns the
/// running QuoteBook; the builder clones it into each snapshot.
pub struct Throttle {
    cfg: ThrottleConfig,
    book: QuoteBook,
    dirty: Vec<QuoteId>,
    last_emit: Option<std::time::Instant>,
}

impl Throttle {
    pub fn new(cfg: ThrottleConfig) -> Self {
        Self { cfg, book: QuoteBook::default(), dirty: Vec::new(), last_emit: None }
    }

    /// Returns true if the quote was accepted (fresh and changed).
    pub fn push(&mut self, q: Quote, now: Timestamp) -> bool {
        if q.is_stale(now, self.cfg.max_quote_age) {
            tracing::debug!(id = %q.id, "dropping stale quote");
            return false;
        }
        let id = q.id.clone();
        if self.book.upsert(q) {
            if !self.dirty.contains(&id) { self.dirty.push(id); }
            true
        } else {
            false
        }
    }

    pub fn ready(&self) -> bool {
        !self.dirty.is_empty()
            && self.last_emit.map_or(true, |t| t.elapsed() >= self.cfg.min_interval)
    }

    /// Force-emit on first call even if nothing moved (initial snapshot).
    pub fn never_emitted(&self) -> bool { self.last_emit.is_none() }

    /// Take the current book + dirty set; resets dirty and the timer.
    pub fn drain(&mut self) -> (QuoteBook, Vec<QuoteId>) {
        self.last_emit = Some(std::time::Instant::now());
        (self.book.clone(), std::mem::take(&mut self.dirty))
    }

    pub fn book(&self) -> &QuoteBook { &self.book }
}
