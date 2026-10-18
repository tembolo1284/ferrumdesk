//! Shared domain types for ferrumdesk.
//!
//! Deliberately has no dependency on ferrumquant or on the UI framework —
//! conversion to `fq-*` types lives in `fd-pricing`, rendering lives in `fd-ui`.

pub mod events;
pub mod ids;
pub mod instrument;
pub mod position;
pub mod quote;
pub mod risk;

pub use events::{Event, MarketEvent, PositionEvent, RiskEvent};
pub use ids::{InstrumentId, PositionId, QuoteId, TradeId};
pub use instrument::{
    BondFuture, DayCount, Frequency, Instrument, InstrumentKind, OvernightSwap, PayReceive,
    Swaption, Tenor, TreasuryBond,
};
pub use position::{Position, Side, Trade};
pub use quote::{Quote, QuoteKind, QuoteSource, Timestamp};
pub use risk::{BucketedDv01, RiskError, RiskResult};

/// Crate-wide error for parsing/validation of domain values.
#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    #[error("invalid tenor: {0}")]
    Tenor(String),
    #[error("invalid identifier: {0}")]
    Id(String),
}
