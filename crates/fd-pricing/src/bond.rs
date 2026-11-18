use crate::pricer::{PriceCtx, Valuation};
use crate::quote_keys;
use crate::schedule::{backward, months_per_period, year_fraction};
use fd_types::{QuoteKind, RiskError, TreasuryBond};

pub struct BondCashflows {
    /// (payment date, amount per 100 face) excluding past flows
    pub flows: Vec<(chrono::NaiveDate, f64)>,
    pub accrued: f64,
}

pub fn cashflows(b: &TreasuryBond, settle: chrono::NaiveDate) -> BondCashflows {
    let sched = backward(b.issue_date, b.maturity, b.frequency);
    let c = b.coupon * b.face * months_per_period(b.frequency) as f64 / 12.0;
    let mut flows = Vec::new();
    let mut accrued = 0.0;
    for w in sched.windows(2) {
        if w[1] <= settle { continue; }
        if w[0] <= settle {
            accrued = c * year_fraction(b.day_count, w[0], settle) / year_fraction(b.day_count, w[0], w[1]);
        }
        flows.push((w[1], c));
    }
    if let Some(last) = flows.last_mut() { last.1 += b.face; }
    BondCashflows { flows, accrued }
}

/// Street convention: dirty price from yield, compounding = coupon frequency.
pub fn price_from_yield(b: &TreasuryBond, settle: chrono::NaiveDate, y: f64) -> (f64, f64) {
    let cf = cashflows(b, settle);
    let m = 12.0 / months_per_period(b.frequency) as f64;
    let dirty: f64 = cf.flows.iter()
        .map(|(d, a)| a / (1.0 + y / m).powf(m * year_fraction(b.day_count, settle, *d)))
        .sum();
    (dirty, dirty - cf.accrued)
}

pub fn yield_from_price(b: &TreasuryBond, settle: chrono::NaiveDate, clean: f64) -> f64 {
    let (mut lo, mut hi) = (-0.05, 0.30);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if price_from_yield(b, settle, mid).1 > clean { lo = mid } else { hi = mid }
    }
    0.5 * (lo + hi)
}

/// Marks off the bond's own yield/price quote. PV per unit face = dirty/100.
/// DV01 = PV change for a +1bp yield move, per unit face (negative for a long),
/// the same convention as the curve path and riskrt.
pub fn value(b: &TreasuryBond, ctx: &PriceCtx) -> Result<Valuation, RiskError> {
    let settle = ctx.snap.valuation_date + chrono::Duration::days(1); // T+1 UST
    let qid = quote_keys::mark(ctx.id);
    let q = ctx.snap.quotes.get(&qid).ok_or_else(|| RiskError::MissingQuote(qid.to_string()))?;
    let y = match q.kind {
        QuoteKind::Yield => q.mid,
        QuoteKind::Price => yield_from_price(b, settle, q.mid),
        k => return Err(RiskError::Pricer(format!("bond {} quoted as {k:?}", ctx.id))),
    };
    let (dirty, _) = price_from_yield(b, settle, y);
    let (up, _) = price_from_yield(b, settle, y + 1e-4);
    let (dn, _) = price_from_yield(b, settle, y - 1e-4);
    let dv01 = (up - dn) / 2.0 / b.face;
    Ok(Valuation { pv: dirty / b.face, vega: None, fair_quote: Some(y), analytic_dv01: Some(dv01) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fd_types::{DayCount, Frequency};
    #[test]
    fn long_bond_dv01_is_negative_pv_change_for_plus_1bp() {
        let b = TreasuryBond { coupon: 0.0425, issue_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(),
            maturity: chrono::NaiveDate::from_ymd_opt(2036, 8, 15).unwrap(), frequency: Frequency::SemiAnnual,
            day_count: DayCount::ActAct, face: 100.0 };
        let settle = chrono::NaiveDate::from_ymd_opt(2026, 10, 17).unwrap();
        let (p0, _) = price_from_yield(&b, settle, 0.0415);
        let (up, _) = price_from_yield(&b, settle, 0.0416);
        let (dn, _) = price_from_yield(&b, settle, 0.0414);
        let dv01 = (up - dn) / 2.0 / b.face;
        assert!(dv01 < 0.0 && dv01 > -0.001, "{dv01}");
        assert!(p0 > 95.0 && p0 < 105.0);
    }
}
