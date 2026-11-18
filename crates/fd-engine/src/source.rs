//! The seam between the UI and wherever risk comes from.
//!
//! `Engine` (this crate) implements it in-process. A remote implementation
//! (fd-remote) fills the same `EngineState` from a riskrt gRPC stream, so
//! the UI renders one `StateSnapshot` per frame either way.

use crate::state::{EngineState, StateSnapshot};
use crate::EngineError;
use fd_types::{InstrumentId, RiskError, RiskResult, Trade};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMode {
    Local,
    Remote { addr: String },
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub name: String,
    pub mode: SourceMode,
    /// False for a read-only source (riskrt has no UpsertTrade yet); the UI
    /// greys out trade entry rather than failing on click.
    pub supports_booking: bool,
    pub curve_names: Vec<String>,
}

pub trait RiskSource: Send + Sync {
    fn info(&self) -> SourceInfo;

    /// Shared state the source writes into and the UI reads from.
    fn state(&self) -> &EngineState;

    /// One consistent read per UI frame.
    fn frame(&self) -> StateSnapshot { self.state().frame() }

    fn book_trade(&self, t: Trade) -> Result<(), EngineError>;

    /// Ad-hoc price of one instrument against the source's current market.
    fn price(&self, id: &InstrumentId) -> Result<RiskResult, RiskError>;

    fn shutdown(&mut self);
}
