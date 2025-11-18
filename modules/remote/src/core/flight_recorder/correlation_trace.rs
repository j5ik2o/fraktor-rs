//! Correlation trace entries linking transport hops.

use alloc::string::String;

use fraktor_actor_rs::core::event_stream::CorrelationId;

use super::correlation_trace_hop::CorrelationTraceHop;

/// Correlates remoting hops via [`CorrelationId`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorrelationTrace {
  correlation_id: CorrelationId,
  authority:      String,
  hop:            CorrelationTraceHop,
}

impl CorrelationTrace {
  /// Creates a new trace entry.
  #[must_use]
  pub fn new(correlation_id: CorrelationId, authority: impl Into<String>, hop: CorrelationTraceHop) -> Self {
    Self { correlation_id, authority: authority.into(), hop }
  }

  /// Returns the correlation identifier.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Returns the authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns hop classification.
  #[must_use]
  pub const fn hop(&self) -> CorrelationTraceHop {
    self.hop
  }
}
