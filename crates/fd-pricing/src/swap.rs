use crate::pricer::{PriceCtx, Valuation};
use crate::schedule::{backward, year_fraction};
use fd_types::{OvernightSwap, PayReceive, RiskError};

/// Fixed-leg annuity (Σ τ_i·DF_i) and float leg (DF_start − DF_end), single-curve OIS.
pub fn annuity_and_float(s: &OvernightSwap, ctx: &PriceCtx) -> (f64, f64) {
    let lag = chrono::Duration::days(s.payment_lag_days as i64);
    let sched = backward(s.effective, s.maturity, s.fixed_frequency);
    let annuity: f64 = sched.windows(2)
        .map(|w| year_fraction(s.fixed_day_count, w[0], w[1]) * ctx.curve.discount(w[1] + lag))
        .sum();
    // float leg: Σ (DF_i-1/DF_i − 1)·DF_i(+lag) ≈ DF_start − DF_end when lag is small
    let float_leg: f64 = sched.windows(2)
        .map(|w| (ctx.curve.discount(w[0]) / ctx.curve.discount(w[1]) - 1.0) * ctx.curve.discount(w[1] + lag))
        .sum();
    (annuity, float_leg)
}

pub fn par_rate(s: &OvernightSwap, ctx: &PriceCtx) -> f64 {
    let (a, f) = annuity_and_float(s, ctx);
    f / a
}

/// PV per unit notional from the position holder's view.
pub fn value(s: &OvernightSwap, ctx: &PriceCtx) -> Result<Valuation, RiskError> {
    let (annuity, float_leg) = annuity_and_float(s, ctx);
    let fixed_leg = s.fixed_rate * annuity;
    let pv = match s.fixed_leg {
        PayReceive::Receive => fixed_leg - float_leg,
        PayReceive::Pay => float_leg - fixed_leg,
    };
    Ok(Valuation { pv, vega: None, fair_quote: Some(float_leg / annuity), analytic_dv01: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricer::Universe;
    use chrono::NaiveDate;
    use fd_market::{curve::add_tenor, MarketSnapshot, QuoteBook, SimpleOisCurve};
    use fd_types::*;
    use std::{collections::HashMap, sync::Arc};

    #[test]
    fn par_swap_has_zero_pv() {
        let r = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let q = [(Tenor::years(1), 0.04), (Tenor::years(2), 0.041), (Tenor::years(5), 0.042)];
        let c: Arc<dyn fd_market::DiscountCurve> = Arc::new(SimpleOisCurve::bootstrap(r, &q));
        let mut curves = HashMap::new(); curves.insert("SOFR".to_string(), c.clone());
        let snap = MarketSnapshot { seq: 1, ts: chrono::Utc::now(), valuation_date: r, quotes: QuoteBook::default(), curves, dirty: vec![] };
        let u = Universe::default();
        let id = InstrumentId::new("SOFR-5Y");
        let ctx = PriceCtx { snap: &snap, curve: c.as_ref(), universe: &u, id: &id };
        let mut s = OvernightSwap { index: "SOFR".into(), effective: r, maturity: add_tenor(r, Tenor::years(5)),
            fixed_rate: 0.0, fixed_leg: PayReceive::Receive, fixed_frequency: Frequency::Annual,
            fixed_day_count: DayCount::Act365F, float_day_count: DayCount::Act360, payment_lag_days: 0 };
        s.fixed_rate = par_rate(&s, &ctx);
        assert!(value(&s, &ctx).unwrap().pv.abs() < 1e-12);
        assert!((s.fixed_rate - 0.042).abs() < 5e-4);
    }
}
