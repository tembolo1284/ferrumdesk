use fd_types::{Position, PositionId, RiskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Start-of-day marks (PV per unit) captured at session open; the live
/// unrealized P&L is measured against these so intraday moves are visible.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Marks {
    pub sod: HashMap<PositionId, f64>,
}

impl Marks {
    pub fn capture(&mut self, pid: PositionId, pv_per_unit: f64) {
        self.sod.entry(pid).or_insert(pv_per_unit);
    }
}

/// One blotter row: position joined with its latest risk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlLine {
    pub position: PositionId,
    pub instrument: String,
    pub book: String,
    pub net_quantity: f64,
    pub avg_price: f64,
    pub mark: Option<f64>,        // PV per unit, live
    pub pv: Option<f64>,          // position PV
    pub dv01: Option<f64>,
    pub unrealized: Option<f64>,  // (live − SOD) × qty
    pub realized: f64,
    pub error: Option<String>,
}

impl PnlLine {
    pub fn total(&self) -> Option<f64> { self.unrealized.map(|u| u + self.realized) }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PnlSummary {
    pub lines: Vec<PnlLine>,
    pub total_pv: f64,
    pub total_dv01: f64,
    pub total_unrealized: f64,
    pub total_realized: f64,
    pub errors: usize,
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
                net_quantity: p.net_quantity, avg_price: p.avg_price,
                mark: None, pv: None, dv01: None, unrealized: None, realized: p.realized_pnl, error: None,
            };
            match risk.get(&p.id) {
                Some(Ok(r)) => {
                    let scaled = r.scaled(p.net_quantity);
                    line.mark = Some(r.pv);
                    line.pv = Some(scaled.pv);
                    line.dv01 = Some(scaled.dv01);
                    line.unrealized = marks.sod.get(&p.id).map(|sod| (r.pv - sod) * p.net_quantity);
                    s.total_pv += scaled.pv;
                    s.total_dv01 += scaled.dv01;
                    s.total_unrealized += line.unrealized.unwrap_or(0.0);
                }
                Some(Err(e)) => { line.error = Some(e.clone()); s.errors += 1; }
                None => {}
            }
            s.total_realized += p.realized_pnl;
            s.lines.push(line);
        }
        s.lines.sort_by(|a, b| (&a.book, &a.instrument).cmp(&(&b.book, &b.instrument)));
        s
    }
}
