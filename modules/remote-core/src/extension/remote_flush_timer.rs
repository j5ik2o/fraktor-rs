//! Timer descriptor for a remote flush session.

use crate::transport::TransportEndpoint;

/// Timer descriptor emitted when a remote flush session starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFlushTimer {
  authority:   TransportEndpoint,
  flush_id:    u64,
  deadline_ms: u64,
}

impl RemoteFlushTimer {
  pub(crate) const fn new(authority: TransportEndpoint, flush_id: u64, deadline_ms: u64) -> Self {
    Self { authority, flush_id, deadline_ms }
  }

  /// Returns the remote authority associated with this timer.
  #[must_use]
  pub const fn authority(&self) -> &TransportEndpoint {
    &self.authority
  }

  /// Returns the flush session identifier.
  #[must_use]
  pub const fn flush_id(&self) -> u64 {
    self.flush_id
  }

  /// Returns the monotonic deadline in milliseconds.
  #[must_use]
  pub const fn deadline_ms(&self) -> u64 {
    self.deadline_ms
  }
}
