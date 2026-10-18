//! Pricing + risk for ferrumdesk. One `Pricer` entry point; instrument
//! modules know how to compute PV, the driver knows how to turn PV into risk.

pub mod bond;
pub mod bond_future;
pub mod convert;
pub mod jacobian;
pub mod pricer;
pub mod schedule;
pub mod swap;
pub mod swaption;

pub use pricer::{PriceCtx, Pricer, PricerConfig, Universe};
pub use jacobian::JacobianCache;

/// Quote-id conventions, so the UI and feeds agree without a lookup table.
pub mod quote_keys {
    use fd_types::{InstrumentId, QuoteId};
    /// Bond yield / futures price / swaption vol are keyed by instrument id.
    pub fn mark(id: &InstrumentId) -> QuoteId { QuoteId::new(id.as_str()) }
    pub fn vol(id: &InstrumentId) -> QuoteId { QuoteId::new(format!("VOL-{}", id.as_str())) }
}
