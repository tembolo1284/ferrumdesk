//! The engine: market loop + risk loop + shared state. The UI depends on
//! this crate and nothing below it.

pub mod bus;
pub mod risk_loop;
pub mod runtime;
pub mod state;

pub use bus::{Bus, Subscription};
pub use runtime::{Engine, EngineConfig};
pub use state::{EngineState, LogLine, StateSnapshot, Stats};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Market(#[from] fd_market::MarketError),
    #[error(transparent)]
    Book(#[from] fd_portfolio::BookError),
    #[error("engine not running")]
    NotRunning,
}
