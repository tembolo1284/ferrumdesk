use fd_types::{Position, PositionId, RiskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Start-of-day state of one position, in currency. Day P&L is measured from here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SodMark {
    /// Position PV at SOD (per-unit PV × SOD quantity). Zero for a position opened today.
    pub position_pv: f64,
    /// Cash ledger at SOD. Zero for a position opened today.
    pub cash: f64,
}

/// SOD marks by position. The P&L identity used everywhere:
///
///   total P&L = PV(now) + cash(now)                    (since inception)
///   day   P&L = total P&L − (PV(sod) + cash(sod))      (since SOD)
///
/// This is product-agnostic: the cash ledger carries the product-specific
/// economics (bond dirty proceeds, futures cash-equivalent, swaption premium),
/// so intraday adds, reductions and flips need no special cases.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Marks {
    pub sod: HashMap<PositionId, SodMark>,
}

impl Marks {
    /// Record SOD for a position that existed at session start, on its first
    /// pricing. No-op if already marked (positions opened intraday are marked
    /// with zeros when booked, so they never land here).
    pub fn capture_existing(&mut self, p: &Position, pv_per_unit: f64) {
        self.sod.entry(p.id).or_insert(SodMark { position_pv: pv_per_unit * p.net_quantity, cash: p.cash });
    }

    /// A position opened during the session starts the day at nothing.
    pub fn mark_new(&mut self, pid: PositionId) {
        self.sod.entry(pid).or_default();
    }

    pub fn get(&self, pid: &PositionId) -> Option<&SodMark> { self.sod.get(pid) }
}

/// One blotter row: position joined with its latest risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlLine {
    pub position: PositionId,
    pub instrument: String,
    pub book: String,
    pub net_quantity: f64,
    pub avg_price: f64,
    /// Cumulative cash from fills (received > 0).
    pub cash: f64,
    pub mark: Option<f64>,        // PV per unit, live
    pub pv: Option<f64>,          // position PV
    pub dv01: Option<f64>,        // PV change for +1bp
    /// PV + cash, since inception. None until priced.
    pub total_pnl: Option<f64>,
    /// Change in (PV + cash) since SOD. None until priced or if no SOD mark.
    pub day_pnl: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PnlSummary {
    pub lines: Vec<PnlLine>,
    pub total_pv: f64,
    pub total_cash: f64,
    pub total_dv01: f64,
    pub total_pnl: f64,
    pub day_pnl: f64,
    /// Positions with a pricing error.
    pub errors: usize,
    /// Open positions not yet priced against any snapshot.
    pub pending: usize,
}

impl PnlSummary {
    /// `risk` is per-unit risk keyed by position (from the risk loop).
    pub fn build<'a>(
        positions: impl Iterator<Item = &'a Position>,
        risk: &HashMap<PositionId, Result<RiskResult, String>>,
        marks: &Marks,
    ) -> Self {
        let mut s = Self::default();
        for p in positions {
            let mut line = PnlLine {
                position: p.id, instrument: p.instrument.to_string(), book: p.book.clone(),
                net_quantity: p.net_quantity, avg_price: p.avg_price, cash: p.cash,
                mark: None, pv: None, dv01: None, total_pnl: None, day_pnl: None, error: None,
            };
            s.total_cash += p.cash;
            match risk.get(&p.id) {
                Some(Ok(r)) => {
                    let scaled = r.scaled(p.net_quantity);
                    let total = scaled.pv + p.cash;
                    line.mark = Some(r.pv);
                    line.pv = Some(scaled.pv);
                    line.dv01 = Some(scaled.dv01);
                    line.total_pnl = Some(total);
                    line.day_pnl = marks.get(&p.id).map(|m| total - (m.position_pv + m.cash));
                    s.total_pv += scaled.pv;
                    s.total_dv01 += scaled.dv01;
                    s.total_pnl += total;
                    s.day_pnl += line.day_pnl.unwrap_or(0.0);
                }
                Some(Err(e)) => { line.error = Some(e.clone()); s.errors += 1; }
                None => {
                    if p.is_flat() {
                        // flat: P&L is fully in cash, no pricing needed
                        line.total_pnl = Some(p.cash);
                        line.day_pnl = marks.get(&p.id).map(|m| p.cash - (m.position_pv + m.cash));
                        s.total_pnl += p.cash;
                        s.day_pnl += line.day_pnl.unwrap_or(0.0);
                    } else {
                        s.pending += 1;
                    }
                }
            }
            s.lines.push(line);
        }
        s.lines.sort_by(|a, b| (&a.book, &a.instrument).cmp(&(&b.book, &b.instrument)));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risk(id: &str, pv_unit: f64) -> RiskResult {
        RiskResult { instrument: id.into(), pv: pv_unit, dv01: -0.0004, bucketed: Default::default(),
            convexity: None, vega: None, fair_quote: None,
            valuation_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            snapshot_ts: chrono::Utc::now(), elapsed_us: 1 }
    }

    #[test]
    fn bond_day_and_total_pnl() {
        // SOD: long 1mm face bought at dirty 99 → cash -990k, PV 990k. Today: mark 100 → PV 1mm.
        let mut p = Position::open("UST".into(), "B", chrono::Utc::now());
        p.net_quantity = 1_000_000.0; p.cash = -990_000.0;
        let mut marks = Marks::default();
        marks.capture_existing(&p, 0.99);
        let mut r = HashMap::new();
        r.insert(p.id, Ok(risk("UST", 1.00)));
        let s = PnlSummary::build(std::iter::once(&p), &r, &marks);
        let l = &s.lines[0];
        assert!((l.total_pnl.unwrap() - 10_000.0).abs() < 1e-6);
        assert!((l.day_pnl.unwrap() - 10_000.0).abs() < 1e-6);

        // Sell half at dirty 100.5 intraday: cash += 502,500; qty 500k; PV = 500k.
        p.net_quantity = 500_000.0; p.cash += 502_500.0;
        let s = PnlSummary::build(std::iter::once(&p), &r, &marks);
        let l = &s.lines[0];
        // total = 500,000 + (-990,000 + 502,500) = 12,500: 2.5k realised on the half sold + 10k on the half kept
        assert!((l.total_pnl.unwrap() - 12_500.0).abs() < 1e-6, "{:?}", l.total_pnl);
        assert!((l.day_pnl.unwrap() - 12_500.0).abs() < 1e-6);
    }

    #[test]
    fn intraday_position_day_equals_total() {
        let mut p = Position::open("SOFR-5Y".into(), "B", chrono::Utc::now());
        p.net_quantity = 10_000_000.0; p.cash = 0.0;
        let mut marks = Marks::default();
        marks.mark_new(p.id);
        let mut r = HashMap::new();
        r.insert(p.id, Ok(risk("SOFR-5Y", 0.0018)));
        let s = PnlSummary::build(std::iter::once(&p), &r, &marks);
        assert!((s.total_pnl - 18_000.0).abs() < 1e-6);
        assert!((s.day_pnl - 18_000.0).abs() < 1e-6);
        assert_eq!(s.pending, 0);
    }
}
