use crate::bond::{price_from_yield, yield_from_price};
use crate::pricer::{PriceCtx, Valuation};
use crate::quote_keys;
use fd_types::{BondFuture, Instrument, QuoteKind, RiskError, TreasuryBond};

/// CBOT 6% conversion factor (simplified: no rounding of months to quarters).
pub fn conversion_factor(b: &TreasuryBond, delivery: chrono::NaiveDate) -> f64 {
    let settle = delivery;
    let (_, clean) = price_from_yield(b, settle, 0.06);
    clean / b.face
}

/// PV per contract = price × size/100. DV01 per contract = CTD DV01 × size / CF.
/// CTD = min(price − fut×CF) over quoted deliverables; falls back to the first.
pub fn value(f: &BondFuture, ctx: &PriceCtx) -> Result<Valuation, RiskError> {
    let qid = quote_keys::mark(ctx.id);
    let q = ctx.snap.quotes.get(&qid).ok_or_else(|| RiskError::MissingQuote(qid.to_string()))?;
    if q.kind != QuoteKind::FuturesPrice {
        return Err(RiskError::Pricer(format!("future {} quoted as {:?}", ctx.id, q.kind)));
    }
    let fut = q.mid;
    let settle = ctx.snap.valuation_date + chrono::Duration::days(1);

    let mut best: Option<(f64, f64)> = None; // (basis, dv01/CF)
    for did in &f.deliverables {
        let Some(def) = ctx.universe.get(did) else { continue };
        let Instrument::TreasuryBond(b) = &def.instrument else { continue };
        let cf = conversion_factor(b, f.last_delivery);
        let mq = quote_keys::mark(did);
        let y = match ctx.snap.quotes.get(&mq) {
            Some(bq) if bq.kind == QuoteKind::Yield => bq.mid,
            Some(bq) if bq.kind == QuoteKind::Price => yield_from_price(b, settle, bq.mid),
            _ => continue,
        };
        let (_, clean) = price_from_yield(b, settle, y);
        let basis = clean - fut * cf;
        let (up, _) = price_from_yield(b, settle, y + 1e-4);
        let (dn, _) = price_from_yield(b, settle, y - 1e-4);
        let dv01 = (dn - up) / 2.0 / b.face;
        if best.map_or(true, |(bb, _)| basis < bb) { best = Some((basis, dv01 / cf)); }
    }
    let dv01_per_unit = best.map(|b| b.1).unwrap_or(0.0);

    Ok(Valuation {
        pv: fut / 100.0 * f.contract_size,
        vega: None,
        fair_quote: Some(fut),
        analytic_dv01: Some(dv01_per_unit * f.contract_size),
    })
}
