//! The [`Codec`] trait abstracts encode / decode for a specific PDU type.

use bytes::{Bytes, BytesMut};

use crate::core::wire::WireError;

/// Abstract encoder / decoder for a specific PDU type `T`.
///
/// Implementations are zero-sized marker types (e.g. [`crate::core::wire::EnvelopeCodec`])
/// that live next to the corresponding PDU struct. Keeping `Codec` generic over `T`
/// means the future L2 (Pekko Artery TCP wire compatible) codec can be added as a
/// drop-in replacement without touching call sites.
pub trait Codec<T> {
  /// Encodes `value` into `buf`, appending a full length-prefixed frame.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the value cannot be encoded — for example
  /// when the encoded frame would exceed `u32::MAX` bytes
  /// ([`WireError::FrameTooLarge`]) or a nested string is longer than
  /// `u32::MAX` ([`WireError::InvalidFormat`]).
  fn encode(&self, value: &T, buf: &mut BytesMut) -> Result<(), WireError>;

  /// Decodes a single full frame from `buf`, advancing `buf` past the consumed bytes.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the frame is malformed:
  /// [`WireError::Truncated`] if the buffer ended before the full frame,
  /// [`WireError::UnknownVersion`] for an unrecognised version byte,
  /// [`WireError::UnknownKind`] for an unrecognised kind byte,
  /// [`WireError::InvalidFormat`] for structurally invalid content, or
  /// [`WireError::InvalidUtf8`] for non-UTF-8 string payloads.
  fn decode(&self, buf: &mut Bytes) -> Result<T, WireError>;
}
