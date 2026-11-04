//! fd-types ↔ ferrumquant. Only compiled with `--features fq`; the plain
//! build uses the in-crate pricers as the reference implementation.

#[cfg(feature = "fq")]
pub mod fq {
    use fd_market::curve::fq::{to_fq, to_period};
    use fd_types::*;
    use fq_core::time::{Date, DayCount as FqDayCount, Frequency as FqFrequency, Thirty360};
    use fq_rates::swap::{OisSwap, SwapError, SwapSide};

    pub fn day_count(dc: DayCount) -> FqDayCount {
        match dc {
            DayCount::Act360 => FqDayCount::Act360,
            DayCount::Act365F => FqDayCount::Act365Fixed,
            DayCount::ActAct => FqDayCount::ActActIsda,
            DayCount::Thirty360 => FqDayCount::Thirty360(Thirty360::BondBasis),
        }
    }

    pub fn frequency(f: Frequency) -> FqFrequency {
        match f {
            Frequency::Annual => FqFrequency::Annual,
            Frequency::SemiAnnual => FqFrequency::Semiannual,
            Frequency::Quarterly => FqFrequency::Quarterly,
            Frequency::Monthly => FqFrequency::Monthly,
        }
    }

    pub fn side(p: PayReceive) -> SwapSide {
        match p { PayReceive::Pay => SwapSide::PayFixed, PayReceive::Receive => SwapSide::ReceiveFixed }
    }

    pub fn date(d: chrono::NaiveDate) -> Date { to_fq(d) }

    /// fd `OvernightSwap` → fq `OisSwap` on USD SOFR conventions (annual Act/360,
    /// ModFollowing, T+2 pay lag), per unit notional. Matches `OisSwap::usd_sofr`,
    /// which is what riskrt prices with.
    pub fn ois(s: &OvernightSwap, notional: f64) -> Result<OisSwap, SwapError> {
        OisSwap::usd_sofr(to_fq(s.effective), to_fq(s.maturity), side(s.fixed_leg), notional, s.fixed_rate)
    }

    /// Spot-starting swap for a tenor, the way the sim/config quotes them.
    pub fn ois_spot(trade_date: chrono::NaiveDate, tenor: Tenor, p: PayReceive, notional: f64, rate: f64) -> Result<OisSwap, SwapError> {
        OisSwap::usd_sofr_spot(to_fq(trade_date), to_period(tenor), side(p), notional, rate)
    }
}
