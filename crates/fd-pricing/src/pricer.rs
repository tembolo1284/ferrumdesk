use crate::jacobian::JacobianCache;
use fd_market::{DiscountCurve, MarketSnapshot};
use fd_types::instrument::InstrumentDef;
use fd_types::{BucketedDv01, Instrument, InstrumentId, RiskError, RiskResult, Tenor};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Static instrument definitions, shared read-only across threads.
#[derive(Debug, Default, Clone)]
pub struct Universe {
    defs: HashMap<InstrumentId, Arc<InstrumentDef>>,
}

impl Universe {
    pub fn insert(&mut self, d: InstrumentDef) { self.defs.insert(d.id.clone(), Arc::new(d)); }
    pub fn get(&self, id: &InstrumentId) -> Option<&Arc<InstrumentDef>> { self.defs.get(id) }
    pub fn iter(&self) -> impl Iterator<Item = &Arc<InstrumentDef>> { self.defs.values() }
}

#[derive(Debug, Clone)]
pub struct PricerConfig {
    /// Curve name used for discounting/projection, e.g. "SOFR".
    pub curve: String,
    /// Bump size for finite-difference DV01, in bp.
    pub bump_bp: f64,
    /// Central (two-sided) differences for DV01 — 2x cost, better convexity handling.
    pub central: bool,
}

impl Default for PricerConfig {
    fn default() -> Self { Self { curve: "SOFR".into(), bump_bp: 1.0, central: false } }
}

/// Everything an instrument pricer sees. `curve` may be a bumped clone.
pub struct PriceCtx<'a> {
    pub snap: &'a MarketSnapshot,
    pub curve: &'a dyn DiscountCurve,
    pub universe: &'a Universe,
    pub id: &'a InstrumentId,
}

/// Output of an instrument module: PV plus whatever non-rate greeks it has.
#[derive(Debug, Clone, Copy, Default)]
pub struct Valuation {
    pub pv: f64,
    pub vega: Option<f64>,
    pub fair_quote: Option<f64>,
    /// Set by quote-marked instruments (bonds, futures) that don't want curve bumping:
    /// their DV01 per unit, already computed analytically.
    pub analytic_dv01: Option<f64>,
}

pub trait Pricer: Send + Sync {
    fn price(&self, id: &InstrumentId, snap: &MarketSnapshot) -> Result<RiskResult, RiskError>;
}

/// The one real implementation: dispatches on `Instrument` kind, then runs
/// the generic risk driver.
pub struct DeskPricer {
    cfg: PricerConfig,
    universe: Arc<Universe>,
    jac: JacobianCache,
}

impl DeskPricer {
    pub fn new(cfg: PricerConfig, universe: Arc<Universe>) -> Self {
        Self { cfg, universe, jac: JacobianCache::default() }
    }

    fn value(&self, inst: &Instrument, ctx: &PriceCtx) -> Result<Valuation, RiskError> {
        match inst {
            Instrument::OvernightSwap(s) => crate::swap::value(s, ctx),
            Instrument::TreasuryBond(b)  => crate::bond::value(b, ctx),
            Instrument::BondFuture(f)    => crate::bond_future::value(f, ctx),
            Instrument::Swaption(o)      => crate::swaption::value(o, ctx),
        }
    }
}

impl Pricer for DeskPricer {
    fn price(&self, id: &InstrumentId, snap: &MarketSnapshot) -> Result<RiskResult, RiskError> {
        let t0 = Instant::now();
        let def = self.universe.get(id)
            .ok_or_else(|| RiskError::Pricer(format!("unknown instrument {id}")))?;
        let base_curve = snap.curve(&self.cfg.curve)
            .ok_or_else(|| RiskError::NoCurve(self.cfg.curve.clone()))?;

        let ctx = PriceCtx { snap, curve: base_curve.as_ref(), universe: &self.universe, id };
        let base = self.value(&def.instrument, &ctx)?;
        let tenors = base_curve.node_tenors();

        let (dv01, bucketed) = if let Some(adv01) = base.analytic_dv01 {
            // quote-marked: all risk to the nearest bucket by maturity
            let mat = def.instrument.maturity();
            let bucket = nearest_tenor(&tenors, base_curve.reference_date(), mat);
            let mut b = BucketedDv01::default();
            b.buckets.insert(bucket, adv01);
            (adv01, b)
        } else {
            self.curve_risk(&def.instrument, &ctx, base_curve, &tenors, base.pv)?
        };

        Ok(RiskResult {
            instrument: id.clone(),
            pv: base.pv,
            dv01,
            bucketed,
            convexity: None,
            vega: base.vega,
            fair_quote: base.fair_quote,
            valuation_date: snap.valuation_date,
            snapshot_ts: snap.ts,
            elapsed_us: t0.elapsed().as_micros() as u64,
        })
    }
}

impl DeskPricer {
    /// Parallel zero DV01 + bucketed par DV01 via Jacobian.
    fn curve_risk(
        &self, inst: &Instrument, ctx: &PriceCtx, base_curve: &Arc<dyn DiscountCurve>,
        tenors: &[Tenor], pv0: f64,
    ) -> Result<(f64, BucketedDv01), RiskError> {
        let h = self.cfg.bump_bp;
        let pv_on = |c: &dyn DiscountCurve| -> Result<f64, RiskError> {
            let cx = PriceCtx { snap: ctx.snap, curve: c, universe: ctx.universe, id: ctx.id };
            Ok(self.value(inst, &cx)?.pv)
        };
        let diff = |up: f64, dn: Option<f64>| match dn {
            Some(d) => (up - d) / (2.0 * h),
            None => (up - pv0) / h,
        };

        // parallel: zero-rate shift (cheap wrapper, no rebootstrap)
        let up = pv_on(&ZeroShift::parallel(base_curve.clone(), h))?;
        let dn = if self.cfg.central { Some(pv_on(&ZeroShift::parallel(base_curve.clone(), -h))?) } else { None };
        let dv01 = diff(up, dn);

        // zero-bucketed sensitivities at each node
        let mut zero_sens = Vec::with_capacity(tenors.len());
        for &t in tenors {
            let up = pv_on(&ZeroShift::node(base_curve.clone(), t, h))?;
            let dn = if self.cfg.central { Some(pv_on(&ZeroShift::node(base_curve.clone(), t, -h))?) } else { None };
            zero_sens.push(diff(up, dn));
        }

        // map to par-quote buckets: par_sens = J^{-T}·zero_sens, J = d(zero)/d(par)
        let j = self.jac.get_or_build(ctx.snap.seq, base_curve, tenors, h);
        let par_sens = j.zero_to_par(&zero_sens);

        let mut b = BucketedDv01::default();
        for (t, s) in tenors.iter().zip(par_sens) { b.buckets.insert(*t, s); }
        Ok((dv01, b))
    }
}

/// Curve wrapper applying a zero-rate shift without rebootstrapping.
/// `node` shifts are tent-shaped between neighbouring node dates.
pub struct ZeroShift {
    base: Arc<dyn DiscountCurve>,
    kind: ShiftKind,
    bp: f64,
}

enum ShiftKind { Parallel, Node { lo: f64, mid: f64, hi: f64 } }

impl ZeroShift {
    pub fn parallel(base: Arc<dyn DiscountCurve>, bp: f64) -> Self { Self { base, kind: ShiftKind::Parallel, bp } }

    pub fn node(base: Arc<dyn DiscountCurve>, tenor: Tenor, bp: f64) -> Self {
        let r = base.reference_date();
        let tenors = base.node_tenors();
        let i = tenors.iter().position(|t| *t == tenor).expect("tenor is a node");
        let tf = |t: Tenor| fd_market::curve::year_frac(r, fd_market::curve::add_tenor(r, t));
        let mid = tf(tenor);
        let lo = if i == 0 { 0.0 } else { tf(tenors[i - 1]) };
        let hi = if i + 1 == tenors.len() { f64::INFINITY } else { tf(tenors[i + 1]) };
        Self { base, kind: ShiftKind::Node { lo, mid, hi }, bp }
    }

    fn weight(&self, t: f64) -> f64 {
        match self.kind {
            ShiftKind::Parallel => 1.0,
            ShiftKind::Node { lo, mid, hi } => {
                if t <= lo || t >= hi { if t >= mid && hi.is_infinite() { 1.0 } else { 0.0 } }
                else if t <= mid { (t - lo) / (mid - lo) }
                else { (hi - t) / (hi - mid) }
            }
        }
    }
}

impl std::fmt::Debug for ZeroShift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "ZeroShift({} bp)", self.bp) }
}

impl DiscountCurve for ZeroShift {
    fn reference_date(&self) -> chrono::NaiveDate { self.base.reference_date() }
    fn discount(&self, d: chrono::NaiveDate) -> f64 {
        let t = fd_market::curve::year_frac(self.reference_date(), d);
        if t <= 0.0 { return 1.0; }
        self.base.discount(d) * (-self.bp * 1e-4 * self.weight(t) * t).exp()
    }
    fn node_tenors(&self) -> Vec<Tenor> { self.base.node_tenors() }
    fn bumped(&self, tenor: Tenor, bp: f64) -> Box<dyn DiscountCurve> { self.base.bumped(tenor, bp) }
    fn bumped_parallel(&self, bp: f64) -> Box<dyn DiscountCurve> { self.base.bumped_parallel(bp) }
}

pub(crate) fn nearest_tenor(tenors: &[Tenor], r: chrono::NaiveDate, mat: chrono::NaiveDate) -> Tenor {
    let t = fd_market::curve::year_frac(r, mat);
    *tenors.iter().min_by(|a, b| {
        let da = (fd_market::curve::year_frac(r, fd_market::curve::add_tenor(r, **a)) - t).abs();
        let db = (fd_market::curve::year_frac(r, fd_market::curve::add_tenor(r, **b)) - t).abs();
        da.partial_cmp(&db).unwrap()
    }).expect("non-empty tenors")
}
