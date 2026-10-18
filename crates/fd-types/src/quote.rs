use crate::QuoteId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Timestamp = DateTime<Utc>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuoteSource { Replay, Sim, Ws, Manual }

/// What the number means — the curve builder and pricers branch on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuoteKind {
    /// OIS par swap rate, decimal.
    ParRate,
    /// Bond yield-to-maturity, decimal.
    Yield,
    /// Clean price per 100.
    Price,
    /// Futures price (e.g. 110-16 stored as decimal 110.5).
    FuturesPrice,
    /// Overnight fixing, decimal.
    Fixing,
    /// Lognormal/normal vol for swaptions, decimal.
    Vol,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub id: QuoteId,
    pub kind: QuoteKind,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    /// Mid when both sides present; otherwise whichever side exists.
    pub mid: f64,
    pub ts: Timestamp,
    pub source: QuoteSource,
}

impl Quote {
    pub fn two_sided(id: QuoteId, kind: QuoteKind, bid: f64, ask: f64, ts: Timestamp, source: QuoteSource) -> Self {
        Self { id, kind, bid: Some(bid), ask: Some(ask), mid: 0.5 * (bid + ask), ts, source }
    }

    pub fn mid_only(id: QuoteId, kind: QuoteKind, mid: f64, ts: Timestamp, source: QuoteSource) -> Self {
        Self { id, kind, bid: None, ask: None, mid, ts, source }
    }

    pub fn spread(&self) -> Option<f64> {
        Some(self.ask? - self.bid?)
    }

    /// True if older than `max_age` relative to `now` — the stale-quote guard for the throttle.
    pub fn is_stale(&self, now: Timestamp, max_age: chrono::Duration) -> bool {
        now - self.ts > max_age
    }
}
