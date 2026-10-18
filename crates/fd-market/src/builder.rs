use crate::curve::{DiscountCurve, SimpleOisCurve};
use crate::snapshot::{MarketSnapshot, QuoteBook};
use crate::MarketError;
use chrono::NaiveDate;
use fd_types::{QuoteId, Tenor, Timestamp};
use std::collections::HashMap;
use std::sync::Arc;

/// Which quotes make up a curve. From `config/instruments.toml`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CurveSpec {
    pub name: String,                  // "SOFR"
    pub nodes: Vec<(Tenor, QuoteId)>,  // (2Y, "SOFR-OIS-2Y") ...
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bootstrapper { Simple, #[cfg(feature = "fq")] Ferrumquant }

/// Turns a drained QuoteBook into an immutable snapshot with curves built.
pub struct SnapshotBuilder {
    valuation_date: NaiveDate,   // fixed per session
    specs: Vec<CurveSpec>,
    bootstrapper: Bootstrapper,
    seq: u64,
    last_curves: HashMap<String, Arc<dyn DiscountCurve>>,
}

impl SnapshotBuilder {
    pub fn new(valuation_date: NaiveDate, specs: Vec<CurveSpec>, bootstrapper: Bootstrapper) -> Self {
        Self { valuation_date, specs, bootstrapper, seq: 0, last_curves: HashMap::new() }
    }

    pub fn build(&mut self, book: QuoteBook, dirty: Vec<QuoteId>, ts: Timestamp)
        -> Result<Arc<MarketSnapshot>, MarketError>
    {
        self.seq += 1;
        let mut curves = HashMap::with_capacity(self.specs.len());

        for spec in &self.specs {
            let touched = dirty.iter().any(|d| spec.nodes.iter().any(|(_, q)| q == d));
            if !touched {
                if let Some(c) = self.last_curves.get(&spec.name) {
                    curves.insert(spec.name.clone(), Arc::clone(c));
                    continue;
                }
            }
            let mut inputs = Vec::with_capacity(spec.nodes.len());
            for (tenor, qid) in &spec.nodes {
                let mid = book.mid(qid).ok_or_else(|| MarketError::MissingNode(qid.to_string()))?;
                inputs.push((*tenor, mid));
            }
            inputs.sort_by_key(|(t, _)| *t);

            let curve: Arc<dyn DiscountCurve> = match self.bootstrapper {
                Bootstrapper::Simple => Arc::new(SimpleOisCurve::bootstrap(self.valuation_date, &inputs)),
                #[cfg(feature = "fq")]
                Bootstrapper::Ferrumquant => Arc::new(crate::curve::fq::FqOisCurve::bootstrap(self.valuation_date, &inputs)?),
            };
            tracing::trace!(curve = %spec.name, seq = self.seq, "rebuilt");
            curves.insert(spec.name.clone(), curve);
        }

        self.last_curves = curves.clone();
        Ok(Arc::new(MarketSnapshot {
            seq: self.seq,
            ts,
            valuation_date: self.valuation_date,
            quotes: book,
            curves,
            dirty,
        }))
    }
}
