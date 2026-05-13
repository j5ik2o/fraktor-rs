//! Envelope PDU: wire-level representation of an outbound message envelope.

use alloc::string::String;

use bytes::Bytes;

use super::EnvelopePayload;

/// Wire-level representation of a message envelope.
///
/// This is the on-the-wire dual of the higher-level
/// [`crate::envelope::OutboundEnvelope`] / [`crate::envelope::InboundEnvelope`]
/// types: it carries the minimum data needed to round-trip through the codec.
///
/// The 96-bit correlation identifier is split into a 64-bit `hi` and a 32-bit
/// `lo` so that the wire frame round-trips the full precision of
/// `fraktor_actor_core_kernel_rs::event::stream::CorrelationId` without
/// silent truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopePdu {
  recipient_path: String,
  sender_path:    Option<String>,
  correlation_hi: u64,
  correlation_lo: u32,
  /// Raw priority byte (0 = System, 1 = User).
  priority:       u8,
  serializer_id:  u32,
  manifest:       Option<String>,
  payload:        Bytes,
}

impl EnvelopePdu {
  /// Creates a new [`EnvelopePdu`].
  ///
  /// `correlation_hi` and `correlation_lo` together encode the 96-bit
  /// correlation identifier carried by the envelope.
  #[must_use]
  pub fn new(
    recipient_path: String,
    sender_path: Option<String>,
    correlation_hi: u64,
    correlation_lo: u32,
    priority: u8,
    payload: EnvelopePayload,
  ) -> Self {
    Self {
      recipient_path,
      sender_path,
      correlation_hi,
      correlation_lo,
      priority,
      serializer_id: payload.serializer_id,
      manifest: payload.manifest,
      payload: payload.bytes,
    }
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

  /// Returns the serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the optional serializer manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the serialized payload bytes.
  #[must_use]
  pub const fn payload(&self) -> &Bytes {
    &self.payload
  }
}
