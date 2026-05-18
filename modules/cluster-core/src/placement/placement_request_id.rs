//! Placement request identifier for command correlation.

/// Correlation identifier used to match commands and results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlacementRequestId(pub u64);
