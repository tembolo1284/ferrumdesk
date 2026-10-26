use fd_market::MarketSnapshot;
use fd_portfolio::{Book, Marks, PnlSummary};
use fd_types::{PositionId, RiskResult, Timestamp};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Stats {
    pub ticks_received: u64,
    pub ticks_accepted: u64,
    pub snapshots: u64,
    pub reprices: u64,
    pub last_reprice_us: u64,
    pub positions_priced: usize,
    pub feed_connected: bool,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub ts: Timestamp,
    pub level: tracing::Level,
    pub msg: String,
}

/// Everything the UI renders from. Writers hold the lock briefly; readers
/// clone out what they need (snapshots are Arc, so that's cheap).
#[derive(Default)]
pub struct Inner {
    pub snapshot: Option<Arc<MarketSnapshot>>,
    pub book: Book,
    pub marks: Marks,
    /// Per-unit risk keyed by position, for the snapshot seq in `risk_seq`.
    pub risk: HashMap<PositionId, Result<RiskResult, String>>,
    pub risk_seq: u64,
    pub stats: Stats,
    pub log: VecDeque<LogLine>,
}

#[derive(Clone, Default)]
pub struct EngineState(Arc<RwLock<Inner>>);

/// Consistent read: a point-in-time copy for one UI frame.
pub struct StateSnapshot {
    pub snapshot: Option<Arc<MarketSnapshot>>,
    pub pnl: PnlSummary,
    pub risk: HashMap<PositionId, Result<RiskResult, String>>,
    pub risk_seq: u64,
    pub stats: Stats,
    pub book: Book,
}

impl EngineState {
    pub fn new(book: Book, marks: Marks) -> Self {
        let inner = Inner { book, marks, ..Default::default() };
        Self(Arc::new(RwLock::new(inner)))
    }

    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Inner> { self.0.read() }
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, Inner> { self.0.write() }

    /// One cheap-ish clone per frame. Called from the UI thread.
    pub fn frame(&self) -> StateSnapshot {
        let g = self.0.read();
        StateSnapshot {
            snapshot: g.snapshot.clone(),
            pnl: PnlSummary::build(g.book.all_positions(), &g.risk, &g.marks),
            risk: g.risk.clone(),
            risk_seq: g.risk_seq,
            stats: g.stats.clone(),
            book: g.book.clone(),
        }
    }

    pub fn log(&self, level: tracing::Level, msg: impl Into<String>) {
        let mut g = self.0.write();
        if g.log.len() >= 500 { g.log.pop_front(); }
        g.log.push_back(LogLine { ts: chrono::Utc::now(), level, msg: msg.into() });
    }
}
