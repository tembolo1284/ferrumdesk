use crate::{InstrumentId, PositionId, PositionRisk, Quote, Timestamp, Trade};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Market side. The engine coalesces `Tick`s into a `SnapshotReady`
/// at a bounded rate; the UI mostly cares about the latter.
#[derive(Debug, Clone)]
pub enum MarketEvent {
    Tick(Quote),
    /// New immutable snapshot id + timestamp; the snapshot itself is fetched
    /// from EngineState to keep the bus message small.
    SnapshotReady { seq: u64, ts: Timestamp },
    FeedStatus { source: crate::QuoteSource, connected: bool, msg: Option<String> },
}

/// Risk side.
#[derive(Debug, Clone)]
pub enum RiskEvent {
    /// Full book repriced against snapshot `seq`.
    BookRepriced { seq: u64, results: Arc<Vec<PositionRisk>>, elapsed_us: u64 },
    /// Single position repriced (e.g. after a trade, without waiting for next snapshot).
    PositionRepriced { seq: u64, risk: PositionRisk },
}

/// Book side — mirrors Trade/Position mutations so the UI and risk loop
/// both react without polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionEvent {
    TradeBooked(Trade),
    PositionChanged(PositionId),
    PositionClosed(PositionId),
    InstrumentAdded(InstrumentId),
}

/// Single bus type so one `Receiver<Event>` feeds the UI log view.
#[derive(Debug, Clone)]
pub enum Event {
    Market(MarketEvent),
    Risk(RiskEvent),
    Position(PositionEvent),
    Shutdown,
}

impl From<MarketEvent> for Event { fn from(e: MarketEvent) -> Self { Event::Market(e) } }
impl From<RiskEvent> for Event { fn from(e: RiskEvent) -> Self { Event::Risk(e) } }
impl From<PositionEvent> for Event { fn from(e: PositionEvent) -> Self { Event::Position(e) } }
