#[cfg(test)]
#[path = "framing_error_kind_test.rs"]
mod tests;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Sub-classification for `StreamError::Framing` failures.
///
/// Mirrors Apache Pekko's `pekko.stream.scaladsl.Framing.FramingException`
/// message forms. Pekko collapses both shapes into a single free-form
/// `RuntimeException(msg)`, but fraktor-rs lifts them into a typed sub-enum
/// so pattern-matching can discriminate the root cause without string
/// parsing.
///
/// Parity mapping:
///
/// - [`Self::FrameTooLarge`] → `FramingException("Maximum allowed message size is $max but tried to
///   send $actual bytes")` (Framing.scala:196).
/// - [`Self::Malformed`] → `FramingException(msg)` where `msg` describes a decoding anomaly such as
///   a truncated trailing frame (Framing.scala:256).
///
/// The [`Display`] rendering preserves the numeric fields for
/// [`Self::FrameTooLarge`] and reproduces the wrapped message verbatim for
/// [`Self::Malformed`], keeping diagnostic parity with Pekko's original text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FramingErrorKind {
  /// A frame exceeded the configured maximum size.
  ///
  /// `actual` carries the observed size and `max` carries the configured
  /// upper bound, both in bytes.
  FrameTooLarge {
    /// Observed frame size in bytes.
    actual: u64,
    /// Configured maximum frame size in bytes.
    max:    u64,
  },
  /// A frame could not be decoded because the byte stream was malformed.
  ///
  /// Wraps a free-form diagnostic message so callers can surface the
  /// original Pekko-style text verbatim.
  Malformed(String),
}

impl Display for FramingErrorKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::FrameTooLarge { actual, max } => {
        write!(f, "Maximum allowed message size is {max} but tried to send {actual} bytes")
      },
      | Self::Malformed(message) => f.write_str(message),
    }
  }
}
