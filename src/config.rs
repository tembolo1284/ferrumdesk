use anyhow::{Context, Result};
use fd_market::{CurveSpec, SimFeed, ReplayFeed, Feed};
use fd_pricing::Universe;
use fd_types::*;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct DeskToml {
    pub valuation_date: Option<chrono::NaiveDate>,
    #[serde(default = "default_curve")] pub curve: String,
    #[serde(default)] pub feed: FeedToml,
    pub book_file: Option<String>,
    #[serde(default = "default_throttle_ms")] pub throttle_ms: u64,
    #[serde(default)] pub risk_threads: usize,
}
fn default_curve() -> String { "SOFR".into() }
fn default_throttle_ms() -> u64 { 250 }

#[derive(Debug, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeedToml {
    #[default] Sim,
    Replay { path: String, #[serde(default = "one")] speed: f64, #[serde(default)] loop_forever: bool },
}
fn one() -> f64 { 1.0 }

#[derive(Debug, Deserialize)]
pub struct InstrumentsToml {
    pub curve: CurveToml,
    #[serde(default)] pub instrument: Vec<InstrumentToml>,
}

#[derive(Debug, Deserialize)]
pub struct CurveToml { pub name: String, pub nodes: Vec<(String, String)> }

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstrumentToml {
    OvernightSwap { id: String, name: String, tenor: String, fixed_rate: f64,
                    #[serde(default = "recv")] side: String, #[serde(default = "two")] payment_lag_days: u8 },
    TreasuryBond  { id: String, name: String, coupon: f64, issue_date: chrono::NaiveDate, maturity: chrono::NaiveDate },
    Swaption      { id: String, name: String, expiry_tenor: String, swap_tenor: String, strike: f64,
                    #[serde(default = "pay")] side: String },
}
fn recv() -> String { "receive".into() }
fn pay() -> String { "pay".into() }
fn two() -> u8 { 2 }

pub fn load_desk(p: impl AsRef<Path>) -> Result<DeskToml> {
    Ok(toml::from_str(&std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.as_ref().display()))?)?)
}

pub fn load_instruments(p: impl AsRef<Path>, val: chrono::NaiveDate) -> Result<(CurveSpec, Universe)> {
    let t: InstrumentsToml = toml::from_str(&std::fs::read_to_string(&p)?)?;
    let spec = CurveSpec {
        name: t.curve.name.clone(),
        nodes: t.curve.nodes.iter().map(|(tn, q)| Ok((tn.parse::<Tenor>()?, QuoteId::new(q.as_str())))).collect::<Result<_, TypeError>>()?,
    };
    let mut u = Universe::default();
    let side = |s: &str| if s.eq_ignore_ascii_case("pay") { PayReceive::Pay } else { PayReceive::Receive };
    for i in t.instrument {
        let def = match i {
            InstrumentToml::OvernightSwap { id, name, tenor, fixed_rate, side: s, payment_lag_days } => InstrumentDef {
                id: id.as_str().into(), name,
                instrument: Instrument::OvernightSwap(OvernightSwap {
                    index: t.curve.name.clone(), effective: val, maturity: fd_market::curve::add_tenor(val, tenor.parse()?),
                    fixed_rate, fixed_leg: side(&s), fixed_frequency: Frequency::Annual,
                    fixed_day_count: DayCount::Act360, float_day_count: DayCount::Act360, payment_lag_days }) },
            InstrumentToml::TreasuryBond { id, name, coupon, issue_date, maturity } => InstrumentDef {
                id: id.as_str().into(), name,
                instrument: Instrument::TreasuryBond(TreasuryBond { coupon, issue_date, maturity,
                    frequency: Frequency::SemiAnnual, day_count: DayCount::ActAct, face: 100.0 }) },
            InstrumentToml::Swaption { id, name, expiry_tenor, swap_tenor, strike, side: s } => {
                let expiry = fd_market::curve::add_tenor(val, expiry_tenor.parse()?);
                InstrumentDef { id: id.as_str().into(), name,
                    instrument: Instrument::Swaption(Swaption { expiry, strike, cash_settled: true,
                        underlying: OvernightSwap { index: t.curve.name.clone(), effective: expiry,
                            maturity: fd_market::curve::add_tenor(expiry, swap_tenor.parse()?), fixed_rate: strike,
                            fixed_leg: side(&s), fixed_frequency: Frequency::Annual, fixed_day_count: DayCount::Act360,
                            float_day_count: DayCount::Act360, payment_lag_days: 2 } }) }
            }
        };
        u.insert(def);
    }
    Ok((spec, u))
}

pub fn make_feed(f: &FeedToml) -> Box<dyn Feed> {
    match f {
        FeedToml::Sim => Box::new(SimFeed::sofr_demo()),
        FeedToml::Replay { path, speed, loop_forever } => Box::new(ReplayFeed::new(path, *speed, *loop_forever)),
    }
}
