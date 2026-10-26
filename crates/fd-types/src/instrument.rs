use crate::{InstrumentId, TypeError};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Tenor like `3M`, `2Y`, `10Y`. Stored as (count, unit) so it sorts and
/// converts to ferrumquant `Period` without re-parsing strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tenor {
    pub n: u16,
    pub unit: TenorUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TenorUnit { D, W, M, Y }

impl Tenor {
    pub const fn new(n: u16, unit: TenorUnit) -> Self { Self { n, unit } }
    pub const fn years(n: u16) -> Self { Self::new(n, TenorUnit::Y) }
    pub const fn months(n: u16) -> Self { Self::new(n, TenorUnit::M) }

    /// Approximate length in months, for ordering buckets on a risk ladder.
    pub fn approx_months(&self) -> u32 {
        match self.unit {
            TenorUnit::D => (self.n as u32) / 30,
            TenorUnit::W => (self.n as u32 * 7) / 30,
            TenorUnit::M => self.n as u32,
            TenorUnit::Y => self.n as u32 * 12,
        }
    }
}

impl PartialOrd for Tenor {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}
impl Ord for Tenor {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.approx_months().cmp(&o.approx_months()) }
}

impl fmt::Display for Tenor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let u = match self.unit { TenorUnit::D => 'D', TenorUnit::W => 'W', TenorUnit::M => 'M', TenorUnit::Y => 'Y' };
        write!(f, "{}{}", self.n, u)
    }
}

impl FromStr for Tenor {
    type Err = TypeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_uppercase();
        let (num, unit) = s.split_at(s.len().saturating_sub(1));
        let n: u16 = num.parse().map_err(|_| TypeError::Tenor(s.clone()))?;
        let unit = match unit {
            "D" => TenorUnit::D, "W" => TenorUnit::W, "M" => TenorUnit::M, "Y" => TenorUnit::Y,
            _ => return Err(TypeError::Tenor(s.clone())),
        };
        Ok(Tenor { n, unit })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DayCount { Act360, Act365F, ActAct, Thirty360 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Frequency { Annual, SemiAnnual, Quarterly, Monthly }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PayReceive { Pay, Receive }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InstrumentKind { TreasuryBond, BondFuture, OvernightSwap, Swaption }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreasuryBond {
    pub coupon: f64,            // annual, decimal (0.04375)
    pub issue_date: NaiveDate,
    pub maturity: NaiveDate,
    pub frequency: Frequency,   // SemiAnnual for UST
    pub day_count: DayCount,    // ActAct for UST
    pub face: f64,              // 100.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BondFuture {
    pub contract: String,                 // "TYZ6"
    pub expiry: NaiveDate,
    pub first_delivery: NaiveDate,
    pub last_delivery: NaiveDate,
    pub contract_size: f64,               // 100_000
    pub deliverables: Vec<InstrumentId>,  // basket, resolved against the instrument universe
}

/// Vanilla OIS (SOFR) — the only swap type in v1, matching ferrumquant's OIS-only bootstrap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OvernightSwap {
    pub index: String,          // "SOFR"
    pub effective: NaiveDate,
    pub maturity: NaiveDate,
    pub fixed_rate: f64,
    pub fixed_leg: PayReceive,  // from the position holder's perspective
    pub fixed_frequency: Frequency,
    pub fixed_day_count: DayCount,
    pub float_day_count: DayCount,
    pub payment_lag_days: u8,   // 2 for SOFR OIS
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Swaption {
    pub expiry: NaiveDate,
    pub underlying: OvernightSwap,  // fixed_leg here = payer/receiver swaption
    pub strike: f64,
    pub cash_settled: bool,
}

/// Instrument definition. Notional/quantity live on `Position`, not here,
/// so one Instrument can back many positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Instrument {
    TreasuryBond(TreasuryBond),
    BondFuture(BondFuture),
    OvernightSwap(OvernightSwap),
    Swaption(Swaption),
}

impl Instrument {
    pub fn kind(&self) -> InstrumentKind {
        match self {
            Instrument::TreasuryBond(_) => InstrumentKind::TreasuryBond,
            Instrument::BondFuture(_)   => InstrumentKind::BondFuture,
            Instrument::OvernightSwap(_) => InstrumentKind::OvernightSwap,
            Instrument::Swaption(_)     => InstrumentKind::Swaption,
        }
    }

    /// Final cash-flow date, used for bucketing and sorting the blotter.
    pub fn maturity(&self) -> NaiveDate {
        match self {
            Instrument::TreasuryBond(b) => b.maturity,
            Instrument::BondFuture(f)   => f.last_delivery,
            Instrument::OvernightSwap(s) => s.maturity,
            Instrument::Swaption(o)     => o.underlying.maturity,
        }
    }
}

/// Instrument plus its stable key — what the instrument universe stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentDef {
    pub id: InstrumentId,
    pub name: String,            // display: "UST 4.375 05/36"
    pub instrument: Instrument,
}
