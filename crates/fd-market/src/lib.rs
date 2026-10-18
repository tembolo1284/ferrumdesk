//! Market data for ferrumdesk.
//!
//! Pipeline: `Feed` → `Receiver<Quote>` → `Throttle` (coalesce) → `SnapshotBuilder`
//! → `Arc<MarketSnapshot>` (immutable: quotes + bootstrapped curves).
//!
//! Nothing here knows about positions or the UI.

pub mod builder;
pub mod curve;
pub mod feed;
pub mod snapshot;
pub mod throttle;

pub use builder::{CurveSpec, SnapshotBuilder};
pub use curve::{DiscountCurve, SimpleOisCurve};
pub use feed::{Feed, FeedHandle, ReplayFeed, SimFeed};
pub use snapshot::{MarketSnapshot, QuoteBook};
pub use throttle::{Throttle, ThrottleConfig};

#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("csv: {0}")]
    Csv(#[from] csv::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bootstrap failed: {0}")]
    Bootstrap(String),
    #[error("missing quote for curve node: {0}")]
    MissingNode(String),
    #[error("feed: {0}")]
    Feed(String),
}
