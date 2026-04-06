//! Event describing backpressure changes for a remote authority.

use alloc::string::String;

use super::{backpressure_signal::BackpressureSignal, correlation_id::CorrelationId};

/// Snapshot of a backpressure notification emitted to observers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingBackpressureEvent {
  authority:      String,
  signal:         BackpressureSignal,
  correlation_id: CorrelationId,
}

impl RemotingBackpressureEvent {
  /// Creates a new event for the specified authority and signal.
  #[must_use]
  pub fn new(authority: impl Into<String>, signal: BackpressureSignal, correlation_id: CorrelationId) -> Self {
    Self { authority: authority.into(), signal, correlation_id }
  }

  /// Returns the authority identifier.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the backpressure signal.
  #[must_use]
  pub const fn signal(&self) -> BackpressureSignal {
    self.signal
  }

  /// Returns the correlation identifier assigned to this event.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }
}
