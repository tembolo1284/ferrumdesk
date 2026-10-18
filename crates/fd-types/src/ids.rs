use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn from_uuid(u: Uuid) -> Self { Self(u) }
            pub fn as_uuid(&self) -> Uuid { self.0 }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // short form for grids: prefix + first 8 hex chars
                write!(f, "{}-{}", $prefix, &self.0.simple().to_string()[..8])
            }
        }
    };
}

uuid_id!(PositionId, "POS");
uuid_id!(TradeId, "TRD");

/// Human-readable, stable instrument key, e.g. `UST-10Y`, `SOFR-5Y`, `TYZ6`.
/// Keyed by the user/config, not generated — so it's a String newtype rather than a UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentId(String);

impl InstrumentId {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl From<&str> for InstrumentId {
    fn from(s: &str) -> Self { Self::new(s) }
}

/// Market quote key, e.g. `SOFR-OIS-2Y`, `UST-10Y-YLD`. Same shape as InstrumentId
/// but kept distinct so a quote can't be accidentally used as a position key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuoteId(String);

impl QuoteId {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for QuoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl From<&str> for QuoteId {
    fn from(s: &str) -> Self { Self::new(s) }
}
