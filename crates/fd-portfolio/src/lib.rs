//! The book. Knows about positions and trades; knows nothing about curves.
//! Risk numbers arrive from fd-pricing as `RiskResult`s and are joined here.

pub mod aggregate;
pub mod book;
pub mod pnl;
pub mod store;

pub use aggregate::{Aggregate, RollupKey};
pub use book::{Book, BookError};
pub use pnl::{Marks, PnlLine, PnlSummary};
pub use store::{load_book, save_book, BookFile};
