//! Envelope PDU: wire-level representation of an outbound message envelope.

use alloc::string::String;

use bytes::Bytes;

use super::{CompressedText, EnvelopePayload};

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
  recipient_path:      CompressedText,
  sender_path:         Option<CompressedText>,
  correlation_hi:      u64,
  correlation_lo:      u32,
  /// Raw priority byte (0 = System, 1 = User).
  priority:            u8,
  redelivery_sequence: Option<u64>,
  serializer_id:       u32,
  manifest:            Option<CompressedText>,
  payload:             Bytes,
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
      recipient_path: CompressedText::literal(recipient_path),
      sender_path: sender_path.map(CompressedText::literal),
      correlation_hi,
      correlation_lo,
      priority,
      redelivery_sequence: None,
      serializer_id: payload.serializer_id,
      manifest: payload.manifest.map(CompressedText::literal),
      payload: payload.bytes,
    }
  }

  /// Creates a new [`EnvelopePdu`] with pre-encoded compression metadata.
  ///
  /// When `manifest_metadata` is `None`, the manifest carried by `payload` is
  /// used as literal metadata.
  #[must_use]
  pub fn new_with_metadata(
    recipient_path: CompressedText,
    sender_path: Option<CompressedText>,
    correlation_hi: u64,
    correlation_lo: u32,
    priority: u8,
    payload: EnvelopePayload,
    manifest_metadata: Option<CompressedText>,
  ) -> Self {
    let manifest = match manifest_metadata {
      | Some(manifest) => Some(manifest),
      | None => payload.manifest.map(CompressedText::literal),
    };
    Self {
      recipient_path,
      sender_path,
      correlation_hi,
      correlation_lo,
      priority,
      redelivery_sequence: None,
      serializer_id: payload.serializer_id,
      manifest,
      payload: payload.bytes,
    }
  }

  /// Returns a copy carrying the given ACK/NACK redelivery sequence metadata.
  #[must_use]
  pub const fn with_redelivery_sequence(mut self, sequence: Option<u64>) -> Self {
    self.redelivery_sequence = sequence;
    self
  }

  /// Returns the recipient actor path.
  #[must_use]
  pub fn recipient_path(&self) -> &str {
    expect_recipient_path_literal(&self.recipient_path)
  }

  /// Returns the recipient actor path metadata.
  #[must_use]
  pub const fn recipient_path_metadata(&self) -> &CompressedText {
    &self.recipient_path
  }

  /// Returns the sender actor path, if known.
  #[must_use]
  pub fn sender_path(&self) -> Option<&str> {
    self.sender_path.as_ref().map(expect_sender_path_literal)
  }

  /// Returns the sender actor path metadata, if known.
  #[must_use]
  pub const fn sender_path_metadata(&self) -> Option<&CompressedText> {
    self.sender_path.as_ref()
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

  /// Returns the ACK/NACK redelivery sequence metadata.
  #[must_use]
  pub const fn redelivery_sequence(&self) -> Option<u64> {
    self.redelivery_sequence
  }

  /// Returns the serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the optional serializer manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_ref().map(expect_manifest_literal)
  }

  /// Returns the optional serializer manifest metadata.
  #[must_use]
  pub const fn manifest_metadata(&self) -> Option<&CompressedText> {
    self.manifest.as_ref()
  }

  /// Returns the serialized payload bytes.
  #[must_use]
  pub const fn payload(&self) -> &Bytes {
    &self.payload
  }
}

fn expect_recipient_path_literal(metadata: &CompressedText) -> &str {
  match metadata {
    | CompressedText::Literal(literal) => literal.as_str(),
    | CompressedText::TableRef(_) => panic!("recipient_path() called on unresolved compressed table reference"),
  }
}

fn expect_sender_path_literal(metadata: &CompressedText) -> &str {
  match metadata {
    | CompressedText::Literal(literal) => literal.as_str(),
    | CompressedText::TableRef(_) => panic!("sender_path() called on unresolved compressed table reference"),
  }
}

fn expect_manifest_literal(metadata: &CompressedText) -> &str {
  match metadata {
    | CompressedText::Literal(literal) => literal.as_str(),
    | CompressedText::TableRef(_) => panic!("manifest() called on unresolved compressed table reference"),
  }
}
