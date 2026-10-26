use fd_engine::{Engine, EngineConfig};
use fd_market::{CurveSpec, SimFeed};
use fd_pricing::pricer::{DeskPricer, PricerConfig, Universe};
use fd_portfolio::{Book, Marks};
use fd_types::*;
use std::sync::Arc;

#[test]
fn sim_pipeline_reprices_a_swap() {
    let r = chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
    let tenors = ["1M","3M","6M","1Y","2Y","3Y","5Y","7Y","10Y","20Y","30Y"];
    let spec = CurveSpec { name: "SOFR".into(),
        nodes: tenors.iter().map(|t| (t.parse().unwrap(), QuoteId::new(format!("SOFR-OIS-{t}")))).collect() };

    let mut u = Universe::default();
    u.insert(InstrumentDef { id: "SOFR-5Y".into(), name: "SOFR 5Y OIS".into(),
        instrument: Instrument::OvernightSwap(OvernightSwap {
            index: "SOFR".into(), effective: r, maturity: fd_market::curve::add_tenor(r, Tenor::years(5)),
            fixed_rate: 0.0375, fixed_leg: PayReceive::Receive, fixed_frequency: Frequency::Annual,
            fixed_day_count: DayCount::Act360, float_day_count: DayCount::Act360, payment_lag_days: 2 }) });

    let pricer = Arc::new(DeskPricer::new(PricerConfig::default(), Arc::new(u)));
    let cfg = EngineConfig { valuation_date: r, curves: vec![spec], ..Default::default() };
    let eng = Engine::start(cfg, Box::new(SimFeed::sofr_demo()), pricer, Book::new(), Marks::default()).unwrap();

    eng.book_trade(Trade { id: TradeId::new(), instrument: "SOFR-5Y".into(), side: Side::Buy,
        quantity: 10_000_000.0, price: 0.0375, ts: chrono::Utc::now(), trader: "paul".into(), book: "RATES".into() }).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1500));
    let f = eng.state.frame();
    assert!(f.stats.snapshots > 0 && f.stats.reprices > 0, "{:?}", f.stats);
    let line = &f.pnl.lines[0];
    assert!(line.dv01.is_some(), "risk missing: {:?}", line.error);
    println!("pv={:?} dv01={:?} reprice_us={}", line.pv, line.dv01, f.stats.last_reprice_us);
}
