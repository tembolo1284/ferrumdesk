use crate::bus::{Bus, Subscription};
use crate::state::EngineState;
use fd_market::MarketSnapshot;
use fd_pricing::Pricer;
use fd_types::{Event, MarketEvent, PositionEvent, PositionRisk, RiskEvent};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Reprices the whole book on every new snapshot, or when a trade changes
/// the book against the current snapshot. Coalesces: if several events
/// arrive while a reprice is running, only one more reprice happens.
pub fn run(
    state: EngineState,
    bus: Bus,
    sub: Subscription,
    pricer: Arc<dyn Pricer>,
    stop: Arc<AtomicBool>,
) {
    let mut dirty = false;
    while !stop.load(Ordering::Relaxed) {
        match sub.recv_timeout(Duration::from_millis(50)) {
            Ok(Event::Market(MarketEvent::SnapshotReady { .. })) => dirty = true,
            Ok(Event::Position(PositionEvent::PositionChanged(_))) => dirty = true,
            Ok(Event::Shutdown) => return,
            Ok(_) => {}
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        // drain anything queued so we reprice once against the latest snapshot
        while let Ok(ev) = sub.try_recv() {
            if matches!(ev, Event::Market(MarketEvent::SnapshotReady { .. }) | Event::Position(PositionEvent::PositionChanged(_))) { dirty = true; }
            if matches!(ev, Event::Shutdown) { return; }
        }
        if dirty {
            dirty = false;
            reprice(&state, &bus, pricer.as_ref());
        }
    }
}

fn reprice(state: &EngineState, bus: &Bus, pricer: &dyn Pricer) {
    let t0 = Instant::now();
    let (snap, positions): (Arc<MarketSnapshot>, Vec<_>) = {
        let g = state.read();
        let Some(s) = g.snapshot.clone() else { return };
        (s, g.book.open_positions().map(|p| (p.id, p.instrument.clone())).collect())
    };

    let results: Vec<PositionRisk> = positions
        .par_iter()
        .map(|(pid, inst)| PositionRisk { position: *pid, result: pricer.price(inst, &snap) })
        .collect();

    let elapsed_us = t0.elapsed().as_micros() as u64;
    let n = results.len();
    {
        let mut g = state.write();
        g.risk.clear();
        for r in &results {
            if let Ok(ok) = &r.result { g.marks.capture(r.position, ok.pv); }
            g.risk.insert(r.position, r.result.clone().map_err(|e| e.to_string()));
        }
        g.risk_seq = snap.seq;
        g.stats.reprices += 1;
        g.stats.last_reprice_us = elapsed_us;
        g.stats.positions_priced = n;
    }
    tracing::debug!(seq = snap.seq, n, elapsed_us, "book repriced");
    bus.publish(RiskEvent::BookRepriced { seq: snap.seq, results: Arc::new(results), elapsed_us });
}
