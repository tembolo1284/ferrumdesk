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
    /// Signed cash this fill generated, in currency (received > 0, paid < 0).
    /// Product-specific; computed by the pricing layer at booking time
    /// (`fd_pricing::economics::trade_cash`). Zero for a swap at inception.
    #[serde(default)]
    pub cash: f64,
    pub ts: Timestamp,
    pub trader: String,
    pub book: String,
}

impl Trade {
    pub fn signed_quantity(&self) -> f64 { self.side.sign() * self.quantity }
}

/// Net position in one instrument. `avg_price` is weighted on the open side
/// and is display-only. P&L is NOT derived from prices here: it is
/// `PV(now) + cash`, computed in fd-portfolio from the pricing layer's PV and
/// the cash ledger below, so every product uses the same formula.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub id: PositionId,
    pub instrument: InstrumentId,
    pub book: String,
    /// Signed: >0 long, <0 short.
    pub net_quantity: f64,
    pub avg_price: f64,
    /// Cumulative signed cash from all fills (received > 0).
    #[serde(default)]
    pub cash: f64,
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
            cash: 0.0,
            trade_count: 0,
            last_update: ts,
        }
    }

    pub fn is_flat(&self) -> bool { self.net_quantity.abs() < 1e-9 }
    pub fn is_long(&self) -> bool { self.net_quantity > 0.0 }

    /// Apply a fill: quantity, display average price, and the cash ledger.
    /// No P&L is computed here.
    pub fn apply(&mut self, t: &Trade) {
        debug_assert_eq!(t.instrument, self.instrument);
        let q = t.signed_quantity();
        let same_direction = self.is_flat() || (self.net_quantity > 0.0) == (q > 0.0);

        if same_direction {
            let new_qty = self.net_quantity + q;
            self.avg_price = (self.avg_price * self.net_quantity + t.price * q) / new_qty;
            self.net_quantity = new_qty;
        } else {
            let dir = self.net_quantity.signum();
            self.net_quantity += q;
            if self.is_flat() {
                self.net_quantity = 0.0;
                self.avg_price = 0.0;
            } else if self.net_quantity.signum() != dir {
                // flipped: remainder opens at the new trade price
                self.avg_price = t.price;
            }
            // reducing: avg_price of the remainder is unchanged
        }
        self.cash += t.cash;
        self.trade_count += 1;
        self.last_update = t.ts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(side: Side, qty: f64, px: f64, cash: f64) -> Trade {
        Trade { id: TradeId::new(), instrument: "X".into(), side, quantity: qty, price: px, cash,
                ts: chrono::Utc::now(), trader: "t".into(), book: "B".into() }
    }

    #[test]
    fn apply_tracks_quantity_avg_and_cash_only() {
        let mut p = Position::open("X".into(), "B", chrono::Utc::now());
        p.apply(&t(Side::Buy, 10.0, 99.0, -990.0));
        p.apply(&t(Side::Buy, 10.0, 101.0, -1010.0));
        assert_eq!(p.net_quantity, 20.0);
        assert!((p.avg_price - 100.0).abs() < 1e-12);
        assert!((p.cash + 2000.0).abs() < 1e-12);
        p.apply(&t(Side::Sell, 5.0, 102.0, 510.0));
        assert_eq!(p.net_quantity, 15.0);
        assert!((p.avg_price - 100.0).abs() < 1e-12, "reduce keeps avg");
        assert!((p.cash + 1490.0).abs() < 1e-12);
        p.apply(&t(Side::Sell, 25.0, 103.0, 2575.0));
        assert_eq!(p.net_quantity, -10.0);
        assert!((p.avg_price - 103.0).abs() < 1e-12, "flip opens at trade px");
    }
}
