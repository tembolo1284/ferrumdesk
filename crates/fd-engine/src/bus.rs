use crossbeam_channel::{Receiver, Sender, TrySendError};
use fd_types::Event;
use parking_lot::Mutex;
use std::sync::Arc;

/// Fan-out event bus. Each subscriber gets its own bounded channel; a slow
/// subscriber drops events rather than stalling the publisher (the UI only
/// needs the latest state anyway — it reads it from EngineState).
#[derive(Clone, Default)]
pub struct Bus {
    subs: Arc<Mutex<Vec<Sender<Event>>>>,
}

pub type Subscription = Receiver<Event>;

impl Bus {
    pub fn new() -> Self { Self::default() }

    pub fn subscribe(&self, capacity: usize) -> Subscription {
        let (tx, rx) = crossbeam_channel::bounded(capacity);
        self.subs.lock().push(tx);
        rx
    }

    pub fn publish(&self, ev: impl Into<Event>) {
        let ev = ev.into();
        let mut subs = self.subs.lock();
        subs.retain(|tx| match tx.try_send(ev.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => { tracing::warn!("subscriber lagging; dropped event"); true }
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    pub fn subscriber_count(&self) -> usize { self.subs.lock().len() }
}
