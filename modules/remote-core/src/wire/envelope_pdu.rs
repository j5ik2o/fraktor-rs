//! Envelope PDU: wire-level representation of an outbound message envelope.

use alloc::string::String;

use bytes::Bytes;

/// Wire-level representation of a message envelope.
///
/// This is the on-the-wire dual of the higher-level
/// [`crate::envelope::OutboundEnvelope`] / [`crate::envelope::InboundEnvelope`]
/// types: it carries the minimum data needed to round-trip through the codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopePdu {
  recipient_path: String,
  sender_path:    Option<String>,
  correlation_id: u64,
  /// Raw priority byte (0 = System, 1 = User).
  priority:       u8,
  payload:        Bytes,
}

impl EnvelopePdu {
  /// Creates a new [`EnvelopePdu`].
  #[must_use]
  pub const fn new(
    recipient_path: String,
    sender_path: Option<String>,
    correlation_id: u64,
    priority: u8,
    payload: Bytes,
  ) -> Self {
    Self { recipient_path, sender_path, correlation_id, priority, payload }
  }

  /// Returns the recipient actor path.
  #[must_use]
  pub fn recipient_path(&self) -> &str {
    &self.recipient_path
  }

  /// Returns the sender actor path, if known.
  #[must_use]
  pub fn sender_path(&self) -> Option<&str> {
    self.sender_path.as_deref()
  }

  /// Returns the correlation id carried by this envelope.
  #[must_use]
  pub const fn correlation_id(&self) -> u64 {
    self.correlation_id
  }

  /// Returns the raw priority byte (0 = System, 1 = User).
  #[must_use]
  pub const fn priority(&self) -> u8 {
    self.priority
  }

  /// Returns the payload bytes.
  #[must_use]
  pub const fn payload(&self) -> &Bytes {
    &self.payload
  }
}
