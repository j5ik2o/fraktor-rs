//! Event describing backpressure changes for a remote authority.

use alloc::string::String;

use super::backpressure_signal::BackpressureSignal;

/// Snapshot of a backpressure notification emitted to observers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingBackpressureEvent {
  authority: String,
  signal:    BackpressureSignal,
}

impl RemotingBackpressureEvent {
  /// Creates a new event for the specified authority and signal.
  #[must_use]
  pub fn new(authority: impl Into<String>, signal: BackpressureSignal) -> Self {
    Self { authority: authority.into(), signal }
  }

  /// Returns the authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the backpressure signal.
  #[must_use]
  pub const fn signal(&self) -> BackpressureSignal {
    self.signal
  }
}
