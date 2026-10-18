use chrono::{Months, NaiveDate};
use fd_types::{DayCount, Frequency};

pub fn months_per_period(f: Frequency) -> u32 {
    match f { Frequency::Annual => 12, Frequency::SemiAnnual => 6, Frequency::Quarterly => 3, Frequency::Monthly => 1 }
}

/// Backward-generated schedule from `maturity`, short first stub. Returns period
/// boundaries including `effective` and `maturity`. No calendar/business-day
/// adjustment in v1 — ferrumquant handles that under the `fq` feature.
pub fn backward(effective: NaiveDate, maturity: NaiveDate, freq: Frequency) -> Vec<NaiveDate> {
    let step = months_per_period(freq);
    let mut dates = vec![maturity];
    let mut k = 1u32;
    loop {
        let d = maturity.checked_sub_months(Months::new(step * k)).expect("date underflow");
        if d <= effective { break; }
        dates.push(d);
        k += 1;
    }
    dates.push(effective);
    dates.reverse();
    dates
}

pub fn year_fraction(dc: DayCount, from: NaiveDate, to: NaiveDate) -> f64 {
    let days = (to - from).num_days() as f64;
    match dc {
        DayCount::Act360 => days / 360.0,
        DayCount::Act365F => days / 365.0,
        DayCount::ActAct => {
            // simple Act/Act ISDA for ≤ 1y periods; good enough for coupon accruals
            let leap = |y: i32| if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366.0 } else { 365.0 };
            use chrono::Datelike;
            if from.year() == to.year() { days / leap(from.year()) }
            else {
                let eoy = NaiveDate::from_ymd_opt(from.year(), 12, 31).unwrap();
                let d1 = (eoy - from).num_days() as f64 + 1.0;
                let soy = NaiveDate::from_ymd_opt(to.year(), 1, 1).unwrap();
                let d2 = (to - soy).num_days() as f64;
                d1 / leap(from.year()) + (to.year() - from.year() - 1) as f64 + d2 / leap(to.year())
            }
        }
        DayCount::Thirty360 => {
            use chrono::Datelike;
            let (d1, d2) = (from.day().min(30) as f64, if from.day() >= 30 { to.day().min(30) } else { to.day() } as f64);
            (360.0 * (to.year() - from.year()) as f64 + 30.0 * (to.month() as f64 - from.month() as f64) + (d2 - d1)) / 360.0
        }
    }
}
