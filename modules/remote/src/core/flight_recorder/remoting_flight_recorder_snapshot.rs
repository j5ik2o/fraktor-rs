//! Snapshot returned by the flight recorder for observability tooling.

use alloc::vec::Vec;

use super::remoting_metric::RemotingMetric;

/// Snapshot returned by the flight recorder for observability tooling.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingFlightRecorderSnapshot {
  pub(super) records: Vec<RemotingMetric>,
}

impl RemotingFlightRecorderSnapshot {
  /// Returns the recorded metrics.
  #[must_use]
  pub fn records(&self) -> &[RemotingMetric] {
    &self.records
  }
}
