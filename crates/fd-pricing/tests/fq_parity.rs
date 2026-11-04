//! Local pricers vs ferrumquant on the same fq-bootstrapped curve.
#![cfg(feature = "fq")]

use chrono::NaiveDate;
use fd_market::curve::fq::{from_fq, FqOisCurve};
use fd_market::{DiscountCurve, MarketSnapshot, QuoteBook};
use fd_pricing::convert::fq as cv;
use fd_pricing::pricer::{DeskPricer, Pricer, PricerConfig, Universe};
use fd_pricing::{swap, PriceCtx};
use fd_types::*;
use fq_rates::swap::OisSwap;
use std::{collections::HashMap, sync::Arc};

fn setup() -> (NaiveDate, Arc<FqOisCurve>, MarketSnapshot) {
    let r = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
    let q = [("1M", 0.0430), ("3M", 0.0425), ("6M", 0.0418), ("1Y", 0.0405), ("2Y", 0.0385),
             ("3Y", 0.0378), ("5Y", 0.0375), ("7Y", 0.0380), ("10Y", 0.0390), ("20Y", 0.0410), ("30Y", 0.0405)];
    let q: Vec<(Tenor, f64)> = q.iter().map(|(t, v)| (t.parse().unwrap(), *v)).collect();
    let c = Arc::new(FqOisCurve::bootstrap(r, &q).unwrap());
    let dyn_c: Arc<dyn DiscountCurve> = c.clone();
    let mut curves = HashMap::new();
    curves.insert("SOFR".to_string(), dyn_c);
    let snap = MarketSnapshot { seq: 1, ts: chrono::Utc::now(), valuation_date: r, quotes: QuoteBook::default(), curves, dirty: vec![] };
    (r, c, snap)
}

/// The fd definition of an fq swap: same effective/maturity, same rate.
fn fd_swap(fq: &OisSwap, rate: f64) -> OvernightSwap {
    OvernightSwap {
        index: "SOFR".into(),
        effective: from_fq(fq.effective_date()),
        maturity: from_fq(fq.maturity_date()),
        fixed_rate: rate, fixed_leg: PayReceive::Receive, fixed_frequency: Frequency::Annual,
        fixed_day_count: DayCount::Act360, float_day_count: DayCount::Act360, payment_lag_days: 2,
    }
}

#[test]
fn swap_pv_and_dv01_match_ferrumquant() {
    let (r, c, snap) = setup();
    let notional = 10_000_000.0;
    let rate = 0.0380;
    let fq_swap = cv::ois_spot(r, Tenor::years(5), PayReceive::Receive, notional, rate).unwrap();
    let fq_pv = fq_swap.npv(c.index(), c.inner()).unwrap();
    let fq_dv01 = fq_swap.dv01(c.index(), c.inner()).unwrap(); // already per +1bp

    let s = fd_swap(&fq_swap, rate);
    let u = Universe::default();
    let id = InstrumentId::new("SOFR-5Y");
    let ctx = PriceCtx { snap: &snap, curve: c.as_ref(), universe: &u, id: &id };
    let fd_pv = swap::value(&s, &ctx).unwrap().pv * notional;
    eprintln!("5Y rcv {rate}: fd pv {fd_pv:.2}  fq pv {fq_pv:.2}  gap {:.2}", fd_pv - fq_pv);
    // fd: unadjusted annual dates, no calendar. fq: ModFollowing + business-day pay lag.
    // Expect agreement to a couple of bp of notional, not to the cent.
    assert!((fd_pv - fq_pv).abs() < 2.0e-4 * notional, "pv gap {}", fd_pv - fq_pv);

    // Full DeskPricer path (parallel DV01 via ZeroShift on the fq curve) vs fq's own dv01.
    let mut u = Universe::default();
    u.insert(InstrumentDef { id: id.clone(), name: "SOFR 5Y".into(), instrument: Instrument::OvernightSwap(s) });
    let p = DeskPricer::new(PricerConfig::default(), Arc::new(u));
    let risk = p.price(&id, &snap).unwrap().scaled(notional);
    eprintln!("fd dv01/bp {:.2}  fq dv01/bp {:.2}  par ladder sum {:.2}", risk.dv01, fq_dv01, risk.bucketed.total());
    assert!((risk.dv01 - fq_dv01).abs() < 0.02 * fq_dv01.abs(), "dv01 gap {}", risk.dv01 - fq_dv01);
}
