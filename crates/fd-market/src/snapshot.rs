use crate::curve::DiscountCurve;
use fd_types::{Quote, QuoteId, Timestamp};
use std::collections::HashMap;
use std::sync::Arc;

/// Latest quote per id. Cheap to clone-on-write between snapshots.
#[derive(Debug, Clone, Default)]
pub struct QuoteBook {
    quotes: HashMap<QuoteId, Quote>,
}

impl QuoteBook {
    pub fn upsert(&mut self, q: Quote) -> bool {
        let changed = self.quotes.get(&q.id).map_or(true, |old| old.mid != q.mid || old.bid != q.bid || old.ask != q.ask);
        self.quotes.insert(q.id.clone(), q);
        changed
    }
    pub fn get(&self, id: &QuoteId) -> Option<&Quote> { self.quotes.get(id) }
    pub fn mid(&self, id: &QuoteId) -> Option<f64> { self.quotes.get(id).map(|q| q.mid) }
    pub fn iter(&self) -> impl Iterator<Item = &Quote> { self.quotes.values() }
    pub fn len(&self) -> usize { self.quotes.len() }
    pub fn is_empty(&self) -> bool { self.quotes.is_empty() }
}

/// Immutable market state at a point in time. The risk loop prices an entire
/// book against one of these; the UI reads the same Arc. Never mutated.
#[derive(Debug)]
pub struct MarketSnapshot {
    pub seq: u64,
    pub ts: Timestamp,
    pub valuation_date: chrono::NaiveDate,
    pub quotes: QuoteBook,
    /// Keyed by curve name, e.g. "SOFR".
    pub curves: HashMap<String, Arc<dyn DiscountCurve>>,
    /// Quote ids that changed since the previous snapshot — lets the risk loop
    /// skip positions whose inputs didn't move.
    pub dirty: Vec<QuoteId>,
}

impl MarketSnapshot {
    pub fn curve(&self, name: &str) -> Option<&Arc<dyn DiscountCurve>> { self.curves.get(name) }
}
