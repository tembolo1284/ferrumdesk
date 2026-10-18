use crate::{InstrumentId, PositionId, Timestamp, TradeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side { Buy, Sell }

impl Side {
    pub fn sign(&self) -> f64 {
        match self { Side::Buy => 1.0, Side::Sell => -1.0 }
    }
}

/// A single fill. Units of `quantity` depend on the instrument:
/// bonds = face amount, futures = contracts, swaps/swaptions = notional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: TradeId,
    pub instrument: InstrumentId,
    pub side: Side,
    pub quantity: f64,
    /// Price for bonds/futures, fixed rate for swaps, premium for swaptions.
    pub price: f64,
    pub ts: Timestamp,
    pub trader: String,
    pub book: String,
}

impl Trade {
    pub fn signed_quantity(&self) -> f64 { self.side.sign() * self.quantity }
}

/// Net position in one instrument. `avg_price` is weighted on the open side;
/// `realized_pnl` accumulates when a trade reduces the position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: PositionId,
    pub instrument: InstrumentId,
    pub book: String,
    /// Signed: >0 long, <0 short.
    pub net_quantity: f64,
    pub avg_price: f64,
    pub realized_pnl: f64,
    pub trade_count: u32,
    pub last_update: Timestamp,
}

impl Position {
    pub fn open(instrument: InstrumentId, book: impl Into<String>, ts: Timestamp) -> Self {
        Self {
            id: PositionId::new(),
            instrument,
            book: book.into(),
            net_quantity: 0.0,
            avg_price: 0.0,
            realized_pnl: 0.0,
            trade_count: 0,
            last_update: ts,
        }
    }

    pub fn is_flat(&self) -> bool { self.net_quantity.abs() < 1e-9 }
    pub fn is_long(&self) -> bool { self.net_quantity > 0.0 }

    /// Apply a fill. Pure position/avg-cost bookkeeping — pricing-aware P&L
    /// (DV01-weighted etc.) lives in fd-portfolio.
    pub fn apply(&mut self, t: &Trade) {
        debug_assert_eq!(t.instrument, self.instrument);
        let q = t.signed_quantity();
        let same_direction = self.is_flat() || (self.net_quantity > 0.0) == (q > 0.0);

        if same_direction {
            let new_qty = self.net_quantity + q;
            self.avg_price = (self.avg_price * self.net_quantity + t.price * q) / new_qty;
            self.net_quantity = new_qty;
        } else {
            // closing (possibly flipping) — realize on the closed portion
            let closed = q.abs().min(self.net_quantity.abs());
            let dir = self.net_quantity.signum();
            self.realized_pnl += dir * (t.price - self.avg_price) * closed;
            self.net_quantity += q;
            if self.is_flat() {
                self.net_quantity = 0.0;
                self.avg_price = 0.0;
            } else if self.net_quantity.signum() != dir {
                // flipped: remainder opens at the new trade price
                self.avg_price = t.price;
            }
        }
        self.trade_count += 1;
        self.last_update = t.ts;
    }
}
