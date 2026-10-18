use super::Feed;
use crate::MarketError;
use chrono::{DateTime, Utc};
use crossbeam_channel::Sender;
use fd_types::{Quote, QuoteId, QuoteKind, QuoteSource};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Replays a CSV of `ts,id,kind,bid,ask` rows, honoring inter-row gaps scaled by `speed`.
/// kind ∈ {par_rate, yield, price, futures_price, fixing, vol}. bid/ask may be blank → mid-only via the other.
pub struct ReplayFeed {
    path: PathBuf,
    speed: f64,      // 1.0 = realtime, 10.0 = 10x, 0.0 = as fast as possible
    loop_forever: bool,
}

impl ReplayFeed {
    pub fn new(path: impl Into<PathBuf>, speed: f64, loop_forever: bool) -> Self {
        Self { path: path.into(), speed, loop_forever }
    }
}

#[derive(serde::Deserialize)]
struct Row { ts: DateTime<Utc>, id: String, kind: String, bid: Option<f64>, ask: Option<f64> }

fn parse_kind(s: &str) -> Option<QuoteKind> {
    Some(match s {
        "par_rate" => QuoteKind::ParRate, "yield" => QuoteKind::Yield, "price" => QuoteKind::Price,
        "futures_price" => QuoteKind::FuturesPrice, "fixing" => QuoteKind::Fixing, "vol" => QuoteKind::Vol,
        _ => return None,
    })
}

impl Feed for ReplayFeed {
    fn name(&self) -> &str { "replay" }

    fn start(self: Box<Self>, tx: Sender<Quote>, stop: Arc<AtomicBool>) -> Result<JoinHandle<()>, MarketError> {
        let mut rows: Vec<Row> = csv::Reader::from_path(&self.path)?
            .deserialize().collect::<Result<_, _>>()?;
        rows.sort_by_key(|r| r.ts);
        if rows.is_empty() { return Err(MarketError::Feed("replay file empty".into())); }

        Ok(std::thread::Builder::new().name("fd-feed-replay".into()).spawn(move || {
            loop {
                let mut prev = rows[0].ts;
                for r in &rows {
                    if stop.load(Ordering::Relaxed) { return; }
                    if self.speed > 0.0 {
                        let gap = (r.ts - prev).to_std().unwrap_or_default();
                        std::thread::sleep(gap.div_f64(self.speed));
                    }
                    prev = r.ts;
                    let Some(kind) = parse_kind(&r.kind) else { continue };
                    let id = QuoteId::new(r.id.clone());
                    // re-stamp to now so the throttle's stale check doesn't reject historical data
                    let ts = Utc::now();
                    let q = match (r.bid, r.ask) {
                        (Some(b), Some(a)) => Quote::two_sided(id, kind, b, a, ts, QuoteSource::Replay),
                        (Some(m), None) | (None, Some(m)) => Quote::mid_only(id, kind, m, ts, QuoteSource::Replay),
                        (None, None) => continue,
                    };
                    if tx.send(q).is_err() { return; }
                }
                if !self.loop_forever { return; }
            }
        })?)
    }
}
