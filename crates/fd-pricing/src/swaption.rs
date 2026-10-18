use crate::pricer::{PriceCtx, Valuation};
use crate::quote_keys;
use crate::swap;
use fd_types::{PayReceive, QuoteKind, RiskError, Swaption};
use statrs::distribution::{Continuous, ContinuousCDF, Normal};

/// Bachelier (normal vol) — the right model for SOFR swaptions; vol quote in
/// decimal (e.g. 0.0085 = 85bp annualised). PV per unit notional.
pub fn value(o: &Swaption, ctx: &PriceCtx) -> Result<Valuation, RiskError> {
    let vid = quote_keys::vol(ctx.id);
    let vq = ctx.snap.quotes.get(&vid).ok_or_else(|| RiskError::MissingQuote(vid.to_string()))?;
    if vq.kind != QuoteKind::Vol {
        return Err(RiskError::Pricer(format!("swaption {} vol quoted as {:?}", ctx.id, vq.kind)));
    }
    let sigma = vq.mid;
    let (annuity, float_leg) = swap::annuity_and_float(&o.underlying, ctx);
    let fwd = float_leg / annuity;
    let t = fd_market::curve::year_frac(ctx.curve.reference_date(), o.expiry);
    if t <= 0.0 { return Ok(Valuation::default()); }

    let n = Normal::new(0.0, 1.0).unwrap();
    let sqt = sigma * t.sqrt();
    let d = (fwd - o.strike) / sqt;
    // payer = fixed_leg Pay: call on the forward rate
    let omega = match o.underlying.fixed_leg { PayReceive::Pay => 1.0, PayReceive::Receive => -1.0 };
    let pv = annuity * (omega * (fwd - o.strike) * n.cdf(omega * d) + sqt * n.pdf(d));
    let vega = annuity * t.sqrt() * n.pdf(d) * 1e-4; // per 1bp of normal vol

    Ok(Valuation { pv, vega: Some(vega), fair_quote: Some(fwd), analytic_dv01: None })
}
