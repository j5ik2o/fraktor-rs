//! Immutable snapshot describing a single metric observation.

use alloc::string::String;

use fraktor_actor_rs::core::event_stream::CorrelationId;

use super::flight_metric_kind::FlightMetricKind;

/// Immutable snapshot describing a single metric observation.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingMetric {
  authority:      String,
  kind:           FlightMetricKind,
  correlation_id: CorrelationId,
  timestamp_ms:   u64,
}

impl RemotingMetric {
  /// Creates a metric entry.
  #[must_use]
  pub fn new(
    authority: impl Into<String>,
    kind: FlightMetricKind,
    correlation_id: CorrelationId,
    timestamp_ms: u64,
  ) -> Self {
    Self { authority: authority.into(), kind, correlation_id, timestamp_ms }
  }

  /// Returns the authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the metric kind.
  #[must_use]
  pub fn kind(&self) -> &FlightMetricKind {
    &self.kind
  }

  /// Returns the recording timestamp.
  #[must_use]
  pub const fn timestamp_ms(&self) -> u64 {
    self.timestamp_ms
  }

  /// Returns the correlation identifier.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }
}
