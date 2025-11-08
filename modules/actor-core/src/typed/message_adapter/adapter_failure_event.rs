//! Event published when a message adapter fails.

#[cfg(test)]
mod tests;

use crate::{actor_prim::Pid, typed::message_adapter::AdapterFailure};

/// Describes a message adaptation failure routed to the event stream.
#[derive(Clone, Debug)]
pub struct AdapterFailureEvent {
  pid:     Pid,
  failure: AdapterFailure,
}

impl AdapterFailureEvent {
  /// Creates a new event for the specified pid and failure reason.
  #[must_use]
  pub const fn new(pid: Pid, failure: AdapterFailure) -> Self {
    Self { pid, failure }
  }

  /// Returns the affected pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the failure details.
  #[must_use]
  pub const fn failure(&self) -> &AdapterFailure {
    &self.failure
  }
}
