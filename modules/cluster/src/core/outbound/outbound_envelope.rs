//! Envelope for outbound cluster messages.

use alloc::{string::String, vec::Vec};

#[cfg(test)]
mod tests;

/// Payload headed to a remote PID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundEnvelope {
  /// Canonical PID string.
  pub pid:     String,
  /// Serialized payload bytes.
  pub payload: Vec<u8>,
}

impl OutboundEnvelope {
  /// Creates a new envelope.
  #[must_use]
  pub const fn new(pid: String, payload: Vec<u8>) -> Self {
    Self { pid, payload }
  }
}
