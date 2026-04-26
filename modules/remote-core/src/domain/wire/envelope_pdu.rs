//! Envelope PDU: wire-level representation of an outbound message envelope.

use alloc::string::String;

use bytes::Bytes;

/// Wire-level representation of a message envelope.
///
/// This is the on-the-wire dual of the higher-level
/// [`crate::domain::envelope::OutboundEnvelope`] / [`crate::domain::envelope::InboundEnvelope`]
/// types: it carries the minimum data needed to round-trip through the codec.
///
/// The 96-bit correlation identifier is split into a 64-bit `hi` and a 32-bit
/// `lo` so that the wire frame round-trips the full precision of
/// `fraktor_actor_core_rs::core::kernel::event::stream::CorrelationId` without
/// silent truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopePdu {
  recipient_path: String,
  sender_path:    Option<String>,
  correlation_hi: u64,
  correlation_lo: u32,
  /// Raw priority byte (0 = System, 1 = User).
  priority:       u8,
  payload:        Bytes,
}

impl EnvelopePdu {
  /// Creates a new [`EnvelopePdu`].
  ///
  /// `correlation_hi` and `correlation_lo` together encode the 96-bit
  /// correlation identifier carried by the envelope.
  #[must_use]
  pub const fn new(
    recipient_path: String,
    sender_path: Option<String>,
    correlation_hi: u64,
    correlation_lo: u32,
    priority: u8,
    payload: Bytes,
  ) -> Self {
    Self { recipient_path, sender_path, correlation_hi, correlation_lo, priority, payload }
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

  /// Returns the high 64 bits of the 96-bit correlation identifier.
  #[must_use]
  pub const fn correlation_hi(&self) -> u64 {
    self.correlation_hi
  }

  /// Returns the low 32 bits of the 96-bit correlation identifier.
  #[must_use]
  pub const fn correlation_lo(&self) -> u32 {
    self.correlation_lo
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
