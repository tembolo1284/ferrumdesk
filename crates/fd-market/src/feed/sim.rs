use super::Feed;
use crate::MarketError;
use chrono::Utc;
use crossbeam_channel::Sender;
use fd_types::{Quote, QuoteId, QuoteKind, QuoteSource};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SimNode {
    pub id: QuoteId,
    pub kind: QuoteKind,
    pub start: f64,
    /// Per-tick std dev in quote units (e.g. 0.0001 = 1bp for rates).
    pub vol: f64,
    /// Half bid/ask width in quote units.
    pub half_spread: f64,
}

/// Random-walk simulator. Emits every node on the first tick so the first
/// snapshot can bootstrap, then random subsets at `interval`.
pub struct SimFeed {
    nodes: Vec<SimNode>,
    interval: Duration,
    seed: u64,
    /// Fraction of nodes that tick each interval.
    hit_rate: f64,
}

impl SimFeed {
    pub fn new(nodes: Vec<SimNode>, interval: Duration, seed: u64, hit_rate: f64) -> Self {
        Self { nodes, interval, seed, hit_rate }
    }

    /// Convenience: a plausible SOFR OIS strip.
    pub fn sofr_demo() -> Self {
        let strip = [("1M", 0.0430), ("3M", 0.0425), ("6M", 0.0418), ("1Y", 0.0405),
                     ("2Y", 0.0385), ("3Y", 0.0378), ("5Y", 0.0375), ("7Y", 0.0380),
                     ("10Y", 0.0390), ("20Y", 0.0410), ("30Y", 0.0405)];
        let nodes = strip.iter().map(|(t, r)| SimNode {
            id: QuoteId::new(format!("SOFR-OIS-{t}")), kind: QuoteKind::ParRate,
            start: *r, vol: 0.00005, half_spread: 0.00025,
        }).collect();
        Self::new(nodes, Duration::from_millis(100), 42, 0.3)
    }
}

impl Feed for SimFeed {
    fn name(&self) -> &str { "sim" }

    fn start(self: Box<Self>, tx: Sender<Quote>, stop: Arc<AtomicBool>) -> Result<JoinHandle<()>, MarketError> {
        Ok(std::thread::Builder::new().name("fd-feed-sim".into()).spawn(move || {
            let mut rng = StdRng::seed_from_u64(self.seed);
            let mut level: Vec<f64> = self.nodes.iter().map(|n| n.start).collect();
            let mut first = true;
            while !stop.load(Ordering::Relaxed) {
                let now = Utc::now();
                for (i, n) in self.nodes.iter().enumerate() {
                    if !first && rng.gen::<f64>() > self.hit_rate { continue; }
                    // gaussian step via Box-Muller
                    let (u1, u2): (f64, f64) = (rng.gen::<f64>().max(1e-12), rng.gen());
                    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                    level[i] += n.vol * z;
                    let q = Quote::two_sided(n.id.clone(), n.kind, level[i] - n.half_spread,
                                             level[i] + n.half_spread, now, QuoteSource::Sim);
                    if tx.send(q).is_err() { return; }
                }
                first = false;
                std::thread::sleep(self.interval);
            }
        })?)
    }
}
