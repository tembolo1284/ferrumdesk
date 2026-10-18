use super::Feed;
use crate::MarketError;
use chrono::Utc;
use crossbeam_channel::Sender;
use fd_types::{Quote, QuoteId, QuoteKind, QuoteSource};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Thin websocket adapter: expects JSON text frames shaped like
/// `{"id":"SOFR-OIS-2Y","kind":"par_rate","bid":0.0384,"ask":0.0386}`.
/// Reconnects with backoff. Swap the frame parser when a real vendor is wired in.
pub struct WsFeed { url: String }

impl WsFeed {
    pub fn new(url: impl Into<String>) -> Self { Self { url: url.into() } }
}

#[derive(serde::Deserialize)]
struct Frame { id: String, kind: QuoteKind, bid: Option<f64>, ask: Option<f64> }

impl Feed for WsFeed {
    fn name(&self) -> &str { "ws" }

    fn start(self: Box<Self>, tx: Sender<Quote>, stop: Arc<AtomicBool>) -> Result<JoinHandle<()>, MarketError> {
        let url = url::Url::parse(&self.url).map_err(|e| MarketError::Feed(e.to_string()))?;
        Ok(std::thread::Builder::new().name("fd-feed-ws".into()).spawn(move || {
            let mut backoff = std::time::Duration::from_millis(500);
            while !stop.load(Ordering::Relaxed) {
                let Ok((mut sock, _)) = tungstenite::connect(url.as_str()) else {
                    tracing::warn!(?backoff, "ws connect failed; retrying");
                    std::thread::sleep(backoff);
                    backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
                    continue;
                };
                backoff = std::time::Duration::from_millis(500);
                tracing::info!("ws connected");
                while !stop.load(Ordering::Relaxed) {
                    let Ok(msg) = sock.read() else { break };
                    let Ok(text) = msg.into_text() else { continue };
                    let Ok(f) = serde_json::from_str::<Frame>(&text) else { continue };
                    let id = QuoteId::new(f.id);
                    let q = match (f.bid, f.ask) {
                        (Some(b), Some(a)) => Quote::two_sided(id, f.kind, b, a, Utc::now(), QuoteSource::Ws),
                        (Some(m), None) | (None, Some(m)) => Quote::mid_only(id, f.kind, m, Utc::now(), QuoteSource::Ws),
                        _ => continue,
                    };
                    if tx.send(q).is_err() { return; }
                }
            }
        })?)
    }
}
