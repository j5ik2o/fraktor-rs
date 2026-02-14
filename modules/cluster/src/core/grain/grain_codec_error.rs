//! Errors raised by grain codecs.

use alloc::string::String;

/// Errors raised during message encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrainCodecError {
  /// The serialization extension is unavailable.
  ExtensionUnavailable {
    /// Failure reason.
    reason: String,
  },
  /// Serializer is not registered for the payload.
  SerializerNotRegistered {
    /// Failure reason.
    reason: String,
  },
  /// Payload is incompatible with the expected format.
  Incompatible {
    /// Failure reason.
    reason: String,
  },
  /// Encoding failed.
  EncodeFailed {
    /// Failure reason.
    reason: String,
  },
  /// Decoding failed.
  DecodeFailed {
    /// Failure reason.
    reason: String,
  },
}
