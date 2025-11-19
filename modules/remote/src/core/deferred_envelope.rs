//! Outbound envelopes queued until associations complete.

use crate::core::RemotingEnvelope;

/// Represents a deferred outbound message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferredEnvelope {
  envelope: RemotingEnvelope,
}

impl DeferredEnvelope {
  /// Creates a new deferred envelope backed by the provided message.
  #[must_use]
  pub fn new(envelope: RemotingEnvelope) -> Self {
    Self { envelope }
  }

  /// Borrows the underlying envelope.
  #[must_use]
  pub fn as_envelope(&self) -> &RemotingEnvelope {
    &self.envelope
  }

  /// Consumes the wrapper and returns the owned envelope.
  #[must_use]
  pub fn into_envelope(self) -> RemotingEnvelope {
    self.envelope
  }
}
