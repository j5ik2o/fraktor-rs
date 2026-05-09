//! Lightweight envelope wrapping a user-level [`AnyMessage`].
//!
//! The dispatcher-pekko-1n-redesign change moves dispatch / mailbox layers from
//! `AnyMessage` to a thin `Envelope` wrapper. The wrapper is intentionally
//! minimal in this change: it carries only the payload, leaving sender,
//! receiver, priority, and correlation metadata to be added in follow-up
//! changes if and when concrete callers need them.

use crate::actor::messaging::AnyMessage;

/// Transport wrapper for user-level messages between dispatcher and mailbox.
#[derive(Debug, Clone)]
pub struct Envelope {
  payload: AnyMessage,
}

impl Envelope {
  /// Wraps the supplied payload in an envelope.
  #[must_use]
  pub const fn new(payload: AnyMessage) -> Self {
    Self { payload }
  }

  /// Returns a reference to the underlying payload.
  #[must_use]
  pub const fn payload(&self) -> &AnyMessage {
    &self.payload
  }

  /// Consumes the envelope and yields the payload.
  #[must_use]
  pub fn into_payload(self) -> AnyMessage {
    self.payload
  }
}

impl From<AnyMessage> for Envelope {
  fn from(payload: AnyMessage) -> Self {
    Self::new(payload)
  }
}
