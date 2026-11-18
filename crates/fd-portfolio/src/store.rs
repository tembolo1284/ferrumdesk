use crate::book::Book;
use crate::pnl::Marks;
use fd_types::{Position, Trade};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// On-disk shape. Positions are persisted with their ids so SOD marks and
/// UI selections survive a restart; trades are the audit trail.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BookFile {
    pub version: u32,
    pub as_of: Option<chrono::DateTime<chrono::Utc>>,
    pub positions: Vec<Position>,
    pub trades: Vec<Trade>,
    #[serde(default)]
    pub marks: Marks,
}

pub fn save_book(path: impl AsRef<Path>, book: &Book, marks: &Marks) -> std::io::Result<()> {
    let f = BookFile {
        version: 2,
        as_of: Some(chrono::Utc::now()),
        positions: book.all_positions().cloned().collect(),
        trades: book.trades().to_vec(),
        marks: marks.clone(),
    };
    let tmp = path.as_ref().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&f)?)?;
    std::fs::rename(tmp, path)   // atomic on the same filesystem
}

pub fn load_book(path: impl AsRef<Path>) -> std::io::Result<(Book, Marks)> {
    let bytes = std::fs::read(path)?;
    match serde_json::from_slice::<BookFile>(&bytes) {
        Ok(f) if f.version >= 2 => Ok((Book::from_parts(f.positions, f.trades), f.marks)),
        _ => {
            // v1 files carried per-unit SOD marks and realized_pnl; neither maps onto
            // the cash ledger, so rebuild positions from the trade log and start marks fresh.
            #[derive(Deserialize)]
            struct Legacy { trades: Vec<Trade> }
            let l: Legacy = serde_json::from_slice(&bytes)?;
            tracing::warn!("book file is v1: rebuilt from {} trades with zero cash; SOD marks reset", l.trades.len());
            Ok((Book::from_trades(l.trades), Marks::default()))
        }
    }
}
