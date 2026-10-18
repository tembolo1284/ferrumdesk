use chrono::NaiveDate;
use fd_types::Tenor;
use std::fmt::Debug;

/// What pricers need from a curve. Object-safe so snapshots can hold
/// `Arc<dyn DiscountCurve>` regardless of which bootstrapper built it.
pub trait DiscountCurve: Debug + Send + Sync {
    fn reference_date(&self) -> NaiveDate;
    fn discount(&self, d: NaiveDate) -> f64;

    /// Continuously-compounded zero rate, Act/365F from reference date.
    fn zero_rate(&self, d: NaiveDate) -> f64 {
        let t = year_frac(self.reference_date(), d);
        if t <= 0.0 { return 0.0; }
        -self.discount(d).ln() / t
    }

    /// Nodes used to build the curve — these define the bucketed-DV01 ladder.
    fn node_tenors(&self) -> Vec<Tenor>;

    /// Same curve with node `tenor`'s input quote bumped by `bp` basis points.
    /// Builder-specific, so each impl owns it (the fq impl uses the Jacobian path).
    fn bumped(&self, tenor: Tenor, bp: f64) -> Box<dyn DiscountCurve>;

    /// Parallel bump of every input node.
    fn bumped_parallel(&self, bp: f64) -> Box<dyn DiscountCurve>;
}

pub fn year_frac(from: NaiveDate, to: NaiveDate) -> f64 {
    (to - from).num_days() as f64 / 365.0
}

pub fn add_tenor(d: NaiveDate, t: Tenor) -> NaiveDate {
    use fd_types::instrument::TenorUnit::*;
    match t.unit {
        D => d + chrono::Duration::days(t.n as i64),
        W => d + chrono::Duration::weeks(t.n as i64),
        M => d.checked_add_months(chrono::Months::new(t.n as u32)).expect("date overflow"),
        Y => d.checked_add_months(chrono::Months::new(t.n as u32 * 12)).expect("date overflow"),
    }
}

/// Minimal OIS bootstrap: annual fixed vs daily-compounded float, treated as
/// par swaps with annual fixed payments, log-linear DF interpolation.
/// Enough for sim/replay and as a sanity check against ferrumquant.
#[derive(Debug, Clone)]
pub struct SimpleOisCurve {
    ref_date: NaiveDate,
    /// (tenor, maturity date, par rate input, bootstrapped DF)
    nodes: Vec<(Tenor, NaiveDate, f64, f64)>,
}

impl SimpleOisCurve {
    /// `quotes` must be sorted by tenor; par rates as decimals.
    pub fn bootstrap(ref_date: NaiveDate, quotes: &[(Tenor, f64)]) -> Self {
        let mut nodes: Vec<(Tenor, NaiveDate, f64, f64)> = Vec::with_capacity(quotes.len());
        for &(tenor, rate) in quotes {
            let mat = add_tenor(ref_date, tenor);
            let t = year_frac(ref_date, mat);
            let df = if t <= 1.0 {
                // single-period: DF = 1 / (1 + r*t)
                1.0 / (1.0 + rate * t)
            } else {
                // annual fixed coupons: sum DF at prior annual dates (interpolated), solve last DF
                let n_full = t.floor() as u32;
                let mut annuity = 0.0;
                let mut tmp = Self { ref_date, nodes: nodes.clone() };
                for k in 1..n_full {
                    let dk = add_tenor(ref_date, Tenor::years(k as u16));
                    annuity += tmp.discount_interp(dk, true);
                }
                let last_accr = t - (n_full - 1) as f64;
                let df = (1.0 - rate * annuity) / (1.0 + rate * last_accr);
                tmp.nodes.push((tenor, mat, rate, df));
                df
            };
            nodes.push((tenor, mat, rate, df));
        }
        Self { ref_date, nodes }
    }

    fn discount_interp(&self, d: NaiveDate, extrapolate: bool) -> f64 {
        if d <= self.ref_date || self.nodes.is_empty() { return 1.0; }
        let t = year_frac(self.ref_date, d);
        let pts: Vec<(f64, f64)> = std::iter::once((0.0, 1.0))
            .chain(self.nodes.iter().map(|n| (year_frac(self.ref_date, n.1), n.3)))
            .collect();
        for w in pts.windows(2) {
            let (t0, d0) = w[0];
            let (t1, d1) = w[1];
            if t <= t1 {
                let a = (t - t0) / (t1 - t0);
                return (d0.ln() * (1.0 - a) + d1.ln() * a).exp();
            }
        }
        if !extrapolate { return *pts.last().map(|p| &p.1).unwrap(); }
        // flat-forward extrapolation off the last two nodes
        let (t0, d0) = pts[pts.len() - 2];
        let (t1, d1) = pts[pts.len() - 1];
        let fwd = -(d1 / d0).ln() / (t1 - t0);
        d1 * (-fwd * (t - t1)).exp()
    }

    pub fn inputs(&self) -> Vec<(Tenor, f64)> {
        self.nodes.iter().map(|n| (n.0, n.2)).collect()
    }
}

impl DiscountCurve for SimpleOisCurve {
    fn reference_date(&self) -> NaiveDate { self.ref_date }
    fn discount(&self, d: NaiveDate) -> f64 { self.discount_interp(d, true) }
    fn node_tenors(&self) -> Vec<Tenor> { self.nodes.iter().map(|n| n.0).collect() }

    fn bumped(&self, tenor: Tenor, bp: f64) -> Box<dyn DiscountCurve> {
        let q: Vec<_> = self.inputs().into_iter()
            .map(|(t, r)| (t, if t == tenor { r + bp * 1e-4 } else { r }))
            .collect();
        Box::new(Self::bootstrap(self.ref_date, &q))
    }

    fn bumped_parallel(&self, bp: f64) -> Box<dyn DiscountCurve> {
        let q: Vec<_> = self.inputs().into_iter().map(|(t, r)| (t, r + bp * 1e-4)).collect();
        Box::new(Self::bootstrap(self.ref_date, &q))
    }
}

#[cfg(feature = "fq")]
pub mod fq {
    //! ferrumquant-backed curve. Three TODOs mark where fq-rates' real API goes.
    use super::*;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    pub struct FqOisCurve {
        ref_date: NaiveDate,
        inputs: Vec<(Tenor, f64)>,
        inner: Arc<fq_rates::OisCurve>, // TODO(fq): actual bootstrapped curve type
    }

    impl FqOisCurve {
        pub fn bootstrap(ref_date: NaiveDate, quotes: &[(Tenor, f64)]) -> Result<Self, crate::MarketError> {
            // TODO(fq): map (Tenor, f64) -> fq_rates::OisHelper, call fq_rates::bootstrap_ois(ref_date, helpers)
            let inner = fq_rates::bootstrap_ois(ref_date, quotes)
                .map_err(|e| crate::MarketError::Bootstrap(e.to_string()))?;
            Ok(Self { ref_date, inputs: quotes.to_vec(), inner: Arc::new(inner) })
        }
    }

    impl DiscountCurve for FqOisCurve {
        fn reference_date(&self) -> NaiveDate { self.ref_date }
        fn discount(&self, d: NaiveDate) -> f64 { self.inner.discount(d) } // TODO(fq): exact accessor
        fn node_tenors(&self) -> Vec<Tenor> { self.inputs.iter().map(|q| q.0).collect() }
        fn bumped(&self, tenor: Tenor, bp: f64) -> Box<dyn DiscountCurve> {
            let q: Vec<_> = self.inputs.iter()
                .map(|&(t, r)| (t, if t == tenor { r + bp * 1e-4 } else { r })).collect();
            Box::new(Self::bootstrap(self.ref_date, &q).expect("bumped rebootstrap"))
        }
        fn bumped_parallel(&self, bp: f64) -> Box<dyn DiscountCurve> {
            let q: Vec<_> = self.inputs.iter().map(|&(t, r)| (t, r + bp * 1e-4)).collect();
            Box::new(Self::bootstrap(self.ref_date, &q).expect("bumped rebootstrap"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn flat_curve_reprices_par() {
        let r = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        let q = [(Tenor::years(1), 0.04), (Tenor::years(2), 0.04), (Tenor::years(5), 0.04)];
        let c = SimpleOisCurve::bootstrap(r, &q);
        // 5Y par: sum r*DF_k + DF_5 == 1
        let mut pv = 0.0;
        for k in 1..=5 {
            pv += 0.04 * c.discount(add_tenor(r, Tenor::years(k)));
        }
        pv += c.discount(add_tenor(r, Tenor::years(5)));
        assert_relative_eq!(pv, 1.0, epsilon = 2e-3); // Act/365 vs annual drift tolerance
    }
}
