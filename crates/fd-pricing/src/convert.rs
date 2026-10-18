//! fd-types ↔ ferrumquant. Only compiled with `--features fq`; the plain
//! build uses the in-crate pricers above as the reference implementation.

#[cfg(feature = "fq")]
pub mod fq {
    use fd_types::*;

    pub fn day_count(dc: DayCount) -> fq_core::DayCounter {          // TODO(fq): real enum name
        match dc {
            DayCount::Act360 => fq_core::DayCounter::Actual360,
            DayCount::Act365F => fq_core::DayCounter::Actual365Fixed,
            DayCount::ActAct => fq_core::DayCounter::ActualActualIsda,
            DayCount::Thirty360 => fq_core::DayCounter::Thirty360,
        }
    }

    pub fn frequency(f: Frequency) -> fq_core::Frequency {           // TODO(fq)
        match f {
            Frequency::Annual => fq_core::Frequency::Annual,
            Frequency::SemiAnnual => fq_core::Frequency::Semiannual,
            Frequency::Quarterly => fq_core::Frequency::Quarterly,
            Frequency::Monthly => fq_core::Frequency::Monthly,
        }
    }

    pub fn ois(s: &OvernightSwap) -> fq_rates::OvernightIndexedSwap {  // TODO(fq)
        fq_rates::OvernightIndexedSwap::builder()
            .effective(s.effective).maturity(s.maturity)
            .fixed_rate(s.fixed_rate)
            .fixed_frequency(frequency(s.fixed_frequency))
            .fixed_day_count(day_count(s.fixed_day_count))
            .payment_lag(s.payment_lag_days as u32)
            .payer(matches!(s.fixed_leg, PayReceive::Pay))
            .build()
    }
}
