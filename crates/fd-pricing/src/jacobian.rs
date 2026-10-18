use fd_market::DiscountCurve;
use fd_types::Tenor;
use parking_lot::Mutex;
use std::sync::Arc;

/// J[i][j] = ∂(zero rate at node i) / ∂(par quote j), per bp.
/// Built once per snapshot by rebootstrapping with each par node bumped —
/// n rebootstraps, amortised over every position priced against that snapshot.
#[derive(Debug, Clone)]
pub struct Jacobian {
    n: usize,
    /// Row-major J^T so `zero_to_par` is a plain mat-vec: par = J^T · zero.
    jt: Vec<f64>,
}

impl Jacobian {
    fn build(curve: &Arc<dyn DiscountCurve>, tenors: &[Tenor], h: f64) -> Self {
        let r = curve.reference_date();
        let dates: Vec<_> = tenors.iter().map(|t| fd_market::curve::add_tenor(r, *t)).collect();
        let z0: Vec<f64> = dates.iter().map(|d| curve.zero_rate(*d)).collect();
        let n = tenors.len();
        let mut jt = vec![0.0; n * n];
        for (j, &tj) in tenors.iter().enumerate() {
            let bumped = curve.bumped(tj, h);
            for (i, d) in dates.iter().enumerate() {
                // zero rates move in decimal; express per bp of par bump
                jt[j * n + i] = (bumped.zero_rate(*d) - z0[i]) * 1e4 / h;
            }
        }
        Self { n, jt }
    }

    /// Given sensitivities to 1bp zero bumps at each node, return
    /// sensitivities to 1bp par-quote bumps.  par_k = Σ_i zero_i · ∂zero_i/∂par_k.
    pub fn zero_to_par(&self, zero_sens: &[f64]) -> Vec<f64> {
        debug_assert_eq!(zero_sens.len(), self.n);
        (0..self.n).map(|k| (0..self.n).map(|i| zero_sens[i] * self.jt[k * self.n + i]).sum()).collect()
    }
}

/// One Jacobian per snapshot seq; the risk loop prices a whole book per
/// snapshot, so this is hit once and read many times.
#[derive(Debug, Default)]
pub struct JacobianCache {
    inner: Mutex<Option<(u64, Arc<Jacobian>)>>,
}

impl JacobianCache {
    pub fn get_or_build(&self, seq: u64, curve: &Arc<dyn DiscountCurve>, tenors: &[Tenor], h: f64) -> Arc<Jacobian> {
        let mut g = self.inner.lock();
        if let Some((s, j)) = &*g { if *s == seq { return Arc::clone(j); } }
        let j = Arc::new(Jacobian::build(curve, tenors, h));
        tracing::trace!(seq, n = tenors.len(), "jacobian rebuilt");
        *g = Some((seq, Arc::clone(&j)));
        j
    }
}
