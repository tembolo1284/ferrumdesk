use crate::bus::Bus;
use crate::risk_loop;
use crate::state::EngineState;
use crate::EngineError;
use fd_market::builder::Bootstrapper;
use fd_market::{CurveSpec, Feed, FeedHandle, SnapshotBuilder, Throttle, ThrottleConfig};
use fd_pricing::Pricer;
use fd_portfolio::{Book, Marks};
use fd_types::{MarketEvent, Trade};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub valuation_date: chrono::NaiveDate,
    pub curves: Vec<CurveSpec>,
    pub bootstrapper: Bootstrapper,
    pub throttle: ThrottleConfig,
    pub feed_capacity: usize,
    pub bus_capacity: usize,
    pub risk_threads: usize,  // 0 = rayon default
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            valuation_date: chrono::Utc::now().date_naive(),
            curves: Vec::new(),
            bootstrapper: Bootstrapper::Simple,
            throttle: ThrottleConfig::default(),
            feed_capacity: 4096,
            bus_capacity: 256,
            risk_threads: 0,
        }
    }
}

/// Handle to a running engine. Dropping it stops all threads.
pub struct Engine {
    pub state: EngineState,
    pub bus: Bus,
    stop: Arc<AtomicBool>,
    feed: Option<FeedHandle>,
    threads: Vec<JoinHandle<()>>,
}

impl Engine {
    pub fn start(
        cfg: EngineConfig,
        feed: Box<dyn Feed>,
        pricer: Arc<dyn Pricer>,
        book: Book,
        marks: Marks,
    ) -> Result<Self, EngineError> {
        if cfg.risk_threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new().num_threads(cfg.risk_threads)
                .thread_name(|i| format!("fd-risk-{i}")).build_global();
        }
        let state = EngineState::new(book, marks);
        let bus = Bus::new();
        let stop = Arc::new(AtomicBool::new(false));
        let feed = FeedHandle::spawn(feed, cfg.feed_capacity)?;
        state.write().stats.feed_connected = true;

        // market loop
        let market_thread = {
            let (state, bus, stop) = (state.clone(), bus.clone(), stop.clone());
            let rx = feed.rx.clone();
            let mut throttle = Throttle::new(cfg.throttle.clone());
            let mut builder = SnapshotBuilder::new(cfg.valuation_date, cfg.curves.clone(), cfg.bootstrapper);
            std::thread::Builder::new().name("fd-market".into()).spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match rx.recv_timeout(Duration::from_millis(20)) {
                        Ok(q) => {
                            let now = chrono::Utc::now();
                            let accepted = throttle.push(q.clone(), now);
                            let mut g = state.write();
                            g.stats.ticks_received += 1;
                            if accepted { g.stats.ticks_accepted += 1; }
                            drop(g);
                            if accepted { bus.publish(MarketEvent::Tick(q)); }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            state.write().stats.feed_connected = false;
                            bus.publish(MarketEvent::FeedStatus { source: fd_types::QuoteSource::Manual, connected: false, msg: Some("feed ended".into()) });
                            break;
                        }
                    }
                    if throttle.ready() || (throttle.never_emitted() && !throttle.book().is_empty()) {
                        let (book, dirty) = throttle.drain();
                        let ts = chrono::Utc::now();
                        match builder.build(book, dirty, ts) {
                            Ok(snap) => {
                                let seq = snap.seq;
                                let mut g = state.write();
                                g.snapshot = Some(snap);
                                g.stats.snapshots += 1;
                                drop(g);
                                bus.publish(MarketEvent::SnapshotReady { seq, ts });
                            }
                            Err(e) => { tracing::warn!(%e, "snapshot build failed"); state.log(tracing::Level::WARN, e.to_string()); }
                        }
                    }
                }
            }).expect("spawn market thread")
        };

        // risk loop
        let risk_thread = {
            let (state, bus, stop) = (state.clone(), bus.clone(), stop.clone());
            let sub = bus.subscribe(cfg.bus_capacity);
            std::thread::Builder::new().name("fd-risk".into())
                .spawn(move || risk_loop::run(state, bus, sub, pricer, stop))
                .expect("spawn risk thread")
        };

        Ok(Self { state, bus, stop, feed: Some(feed), threads: vec![market_thread, risk_thread] })
    }

    /// Book a trade from the UI. Publishes position events; the risk loop
    /// picks up PositionChanged and reprices against the current snapshot.
    pub fn book_trade(&self, t: Trade) -> Result<(), EngineError> {
        let events = self.state.write().book.book_trade(t)?;
        for ev in events { self.bus.publish(ev); }
        Ok(())
    }

    pub fn is_running(&self) -> bool { !self.stop.load(Ordering::Relaxed) }

    pub fn shutdown(&mut self) {
        if self.stop.swap(true, Ordering::Relaxed) { return; }
        self.bus.publish(fd_types::Event::Shutdown);
        if let Some(mut f) = self.feed.take() { f.stop(); }
        for t in self.threads.drain(..) { let _ = t.join(); }
        tracing::info!("engine stopped");
    }
}

impl Drop for Engine {
    fn drop(&mut self) { self.shutdown(); }
}
