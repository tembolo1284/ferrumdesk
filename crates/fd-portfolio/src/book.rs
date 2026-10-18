use fd_types::{InstrumentId, Position, PositionEvent, PositionId, Timestamp, Trade};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum BookError {
    #[error("unknown position {0}")]
    UnknownPosition(PositionId),
    #[error("trade quantity must be > 0")]
    BadQuantity,
}

/// One position per (instrument, book-name). Trades are retained for the
/// blotter and for replaying a day's activity on restart.
#[derive(Debug, Default, Clone)]
pub struct Book {
    positions: HashMap<PositionId, Position>,
    by_key: HashMap<(InstrumentId, String), PositionId>,
    trades: Vec<Trade>,
}

impl Book {
    pub fn new() -> Self { Self::default() }

    /// Apply a fill; opens the position if needed. Returns the events the
    /// engine should publish (so UI and risk loop react).
    pub fn book_trade(&mut self, t: Trade) -> Result<Vec<PositionEvent>, BookError> {
        if t.quantity <= 0.0 { return Err(BookError::BadQuantity); }
        let key = (t.instrument.clone(), t.book.clone());
        let pid = match self.by_key.get(&key) {
            Some(p) => *p,
            None => {
                let p = Position::open(t.instrument.clone(), t.book.clone(), t.ts);
                let id = p.id;
                self.positions.insert(id, p);
                self.by_key.insert(key, id);
                id
            }
        };
        let pos = self.positions.get_mut(&pid).expect("just inserted");
        pos.apply(&t);
        let flat = pos.is_flat();
        tracing::info!(trade = %t.id, pos = %pid, qty = pos.net_quantity, "booked");
        self.trades.push(t.clone());

        let mut ev = vec![PositionEvent::TradeBooked(t), PositionEvent::PositionChanged(pid)];
        if flat { ev.push(PositionEvent::PositionClosed(pid)); }
        Ok(ev)
    }

    pub fn position(&self, id: &PositionId) -> Option<&Position> { self.positions.get(id) }

    pub fn position_for(&self, inst: &InstrumentId, book: &str) -> Option<&Position> {
        self.by_key.get(&(inst.clone(), book.to_owned())).and_then(|p| self.positions.get(p))
    }

    /// Live (non-flat) positions — what the risk loop prices.
    pub fn open_positions(&self) -> impl Iterator<Item = &Position> {
        self.positions.values().filter(|p| !p.is_flat())
    }

    pub fn all_positions(&self) -> impl Iterator<Item = &Position> { self.positions.values() }
    pub fn trades(&self) -> &[Trade] { &self.trades }
    pub fn len(&self) -> usize { self.positions.len() }
    pub fn is_empty(&self) -> bool { self.positions.is_empty() }

    /// Distinct instruments with open risk.
    pub fn instruments(&self) -> Vec<InstrumentId> {
        let mut v: Vec<_> = self.open_positions().map(|p| p.instrument.clone()).collect();
        v.sort(); v.dedup(); v
    }

    /// Rebuild from a trade list (restart path). Positions come out with
    /// fresh ids; callers that persisted ids should use `store::load_book`.
    pub fn from_trades(trades: impl IntoIterator<Item = Trade>) -> Self {
        let mut b = Self::new();
        for t in trades { let _ = b.book_trade(t); }
        b
    }

    pub fn last_update(&self) -> Option<Timestamp> {
        self.positions.values().map(|p| p.last_update).max()
    }

    /// Restore persisted positions (keeping their ids) plus the trade log.
    pub fn from_parts(positions: Vec<Position>, trades: Vec<Trade>) -> Self {
        let mut b = Self::new();
        for p in positions {
            b.by_key.insert((p.instrument.clone(), p.book.clone()), p.id);
            b.positions.insert(p.id, p);
        }
        b.trades = trades;
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_types::{Side, TradeId};

    fn t(inst: &str, side: Side, qty: f64, px: f64) -> Trade {
        Trade { id: TradeId::new(), instrument: inst.into(), side, quantity: qty, price: px,
                ts: chrono::Utc::now(), trader: "paul".into(), book: "RATES".into() }
    }

    #[test]
    fn open_close_realizes() {
        let mut b = Book::new();
        b.book_trade(t("UST-10Y", Side::Buy, 10.0, 99.0)).unwrap();
        b.book_trade(t("UST-10Y", Side::Buy, 10.0, 101.0)).unwrap();
        let p = b.position_for(&"UST-10Y".into(), "RATES").unwrap();
        assert_eq!(p.net_quantity, 20.0);
        assert!((p.avg_price - 100.0).abs() < 1e-12);

        let ev = b.book_trade(t("UST-10Y", Side::Sell, 20.0, 102.0)).unwrap();
        let p = b.position_for(&"UST-10Y".into(), "RATES").unwrap();
        assert!(p.is_flat());
        assert!((p.realized_pnl - 40.0).abs() < 1e-12);
        assert!(matches!(ev.last(), Some(PositionEvent::PositionClosed(_))));
        assert_eq!(b.open_positions().count(), 0);
    }
}
