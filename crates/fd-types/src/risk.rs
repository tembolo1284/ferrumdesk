use crate::{InstrumentId, PositionId, Tenor, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum RiskError {
    #[error("missing quote: {0}")]
    MissingQuote(String),
    #[error("curve not available: {0}")]
    NoCurve(String),
    #[error("pricer error: {0}")]
    Pricer(String),
}

/// Par-quote bucketed DV01 (per the curve-Jacobian decision), keyed by tenor
/// so it sorts into a ladder for free. Values are per 1bp, in position currency.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BucketedDv01 {
    pub buckets: BTreeMap<Tenor, f64>,
}

impl BucketedDv01 {
    pub fn total(&self) -> f64 { self.buckets.values().sum() }

    pub fn add_assign(&mut self, other: &Self) {
        for (t, v) in &other.buckets {
            *self.buckets.entry(*t).or_insert(0.0) += v;
        }
    }

    pub fn scaled(&self, k: f64) -> Self {
        Self { buckets: self.buckets.iter().map(|(t, v)| (*t, v * k)).collect() }
    }
}

/// Per-unit risk for one instrument — scale by signed quantity to get position risk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskResult {
    pub instrument: InstrumentId,
    pub pv: f64,
    /// Parallel (zero-rate) DV01 per 1bp.
    pub dv01: f64,
    pub bucketed: BucketedDv01,
    /// Convexity per 1bp², where meaningful.
    pub convexity: Option<f64>,
    /// Vega per 1bp vol for swaptions; None otherwise.
    pub vega: Option<f64>,
    /// Fair value in quote units (yield / par rate / price) for display next to the market.
    pub fair_quote: Option<f64>,
    pub valuation_date: chrono::NaiveDate,
    pub snapshot_ts: Timestamp,
    pub elapsed_us: u64,
}

impl RiskResult {
    /// Position-level risk: all linear measures scale with signed quantity.
    pub fn scaled(&self, signed_qty: f64) -> Self {
        Self {
            pv: self.pv * signed_qty,
            dv01: self.dv01 * signed_qty,
            bucketed: self.bucketed.scaled(signed_qty),
            convexity: self.convexity.map(|c| c * signed_qty),
            vega: self.vega.map(|v| v * signed_qty),
            ..self.clone()
        }
    }
}

/// What the risk loop publishes per position after a snapshot reprice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionRisk {
    pub position: PositionId,
    pub result: Result<RiskResult, RiskError>,
}
