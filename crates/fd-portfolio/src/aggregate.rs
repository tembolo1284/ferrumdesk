use fd_types::{BucketedDv01, InstrumentKind, Position, RiskResult, Tenor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RollupKey {
    Desk,
    Book(String),
    Product(InstrumentKind),
    Instrument(String),
}

/// Aggregated risk for one roll-up bucket — the risk ladder view.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Aggregate {
    pub pv: f64,
    pub dv01: f64,
    pub bucketed: BucketedDv01,
    pub vega: f64,
    pub positions: usize,
}

impl Aggregate {
    pub fn add(&mut self, r: &RiskResult, signed_qty: f64) {
        let s = r.scaled(signed_qty);
        self.pv += s.pv;
        self.dv01 += s.dv01;
        self.bucketed.add_assign(&s.bucketed);
        self.vega += s.vega.unwrap_or(0.0);
        self.positions += 1;
    }

    /// Ladder as (tenor, dv01) with zeros filled for `all_tenors` so the UI
    /// gets fixed columns.
    pub fn ladder(&self, all_tenors: &[Tenor]) -> Vec<(Tenor, f64)> {
        all_tenors.iter().map(|t| (*t, *self.bucketed.buckets.get(t).unwrap_or(&0.0))).collect()
    }
}

/// Roll up a set of (position, per-unit risk, instrument kind) into every key level.
pub fn rollup<'a>(
    rows: impl Iterator<Item = (&'a Position, &'a RiskResult, InstrumentKind)>,
) -> BTreeMap<RollupKey, Aggregate> {
    let mut m: BTreeMap<RollupKey, Aggregate> = BTreeMap::new();
    for (p, r, kind) in rows {
        let q = p.net_quantity;
        for key in [
            RollupKey::Desk,
            RollupKey::Book(p.book.clone()),
            RollupKey::Product(kind),
            RollupKey::Instrument(p.instrument.to_string()),
        ] {
            m.entry(key).or_default().add(r, q);
        }
    }
    m
}
