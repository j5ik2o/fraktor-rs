//! Errors produced by the wire format encode / decode pipeline.

use core::fmt;

/// Errors raised while encoding or decoding a wire PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireError {
  /// The buffer contents are structurally invalid (e.g. length field larger than
  /// the buffer can possibly contain).
  InvalidFormat,
  /// The frame header declares a wire format version that this crate does not
  /// understand.
  UnknownVersion,
  /// The frame header declares a PDU `kind` byte that this crate does not
  /// understand.
  UnknownKind,
  /// The buffer was cut off before the full frame could be read.
  Truncated,
  /// A string payload contained invalid UTF-8.
  InvalidUtf8,
  /// The frame is larger than the caller-allowed limit.
  FrameTooLarge,
}

impl fmt::Display for WireError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | WireError::InvalidFormat => f.write_str("wire: invalid format"),
      | WireError::UnknownVersion => f.write_str("wire: unknown version"),
      | WireError::UnknownKind => f.write_str("wire: unknown kind"),
      | WireError::Truncated => f.write_str("wire: truncated buffer"),
      | WireError::InvalidUtf8 => f.write_str("wire: invalid utf-8"),
      | WireError::FrameTooLarge => f.write_str("wire: frame too large"),
    }
  }
}

impl core::error::Error for WireError {}
