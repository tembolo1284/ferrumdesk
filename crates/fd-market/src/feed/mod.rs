pub mod replay;
pub mod sim;
#[cfg(feature = "ws")]
pub mod ws;

pub use replay::ReplayFeed;
pub use sim::SimFeed;

use crate::MarketError;
use crossbeam_channel::{Receiver, Sender};
use fd_types::Quote;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// A quote source. Implementations spawn their own thread and push into `tx`
/// until `stop` is set. Keeps feeds swappable from config without the engine caring.
pub trait Feed: Send {
    fn name(&self) -> &str;
    fn start(self: Box<Self>, tx: Sender<Quote>, stop: Arc<AtomicBool>) -> Result<JoinHandle<()>, MarketError>;
}

/// Running feed: channel + stop flag + thread handle.
pub struct FeedHandle {
    pub name: String,
    pub rx: Receiver<Quote>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl FeedHandle {
    pub fn spawn(feed: Box<dyn Feed>, capacity: usize) -> Result<Self, MarketError> {
        let (tx, rx) = crossbeam_channel::bounded(capacity);
        let stop = Arc::new(AtomicBool::new(false));
        let name = feed.name().to_owned();
        let join = feed.start(tx, Arc::clone(&stop))?;
        Ok(Self { name, rx, stop, join: Some(join) })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() { let _ = j.join(); }
    }
}

impl Drop for FeedHandle {
    fn drop(&mut self) { self.stop(); }
}
