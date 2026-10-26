pub mod curve;
pub mod log;
pub mod market;
pub mod positions;
pub mod pricer;
pub mod risk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab { Positions, Risk, Market, Curve, Pricer, Log }

impl Tab {
    pub const ALL: [Tab; 6] = [Tab::Positions, Tab::Risk, Tab::Market, Tab::Curve, Tab::Pricer, Tab::Log];
    pub fn label(self) -> &'static str {
        match self {
            Tab::Positions => "Positions", Tab::Risk => "Risk", Tab::Market => "Market",
            Tab::Curve => "Curve", Tab::Pricer => "Pricer", Tab::Log => "Log",
        }
    }
}
