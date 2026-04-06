//! Error type produced by [`crate::tcp_transport::WireFrameCodec`].

use std::{fmt, io};

use fraktor_remote_core_rs::wire::WireError;

/// Error returned by [`crate::tcp_transport::WireFrameCodec`] when used as a
/// `tokio_util::codec::{Encoder, Decoder}`.
///
/// `tokio_util::codec::{Encoder, Decoder}` require their `Error` associated
/// type to implement `From<std::io::Error>`. The no_std-friendly
/// [`WireError`] from `remote-core` cannot depend on `io::Error`, so we wrap
/// both here.
#[derive(Debug)]
pub enum FrameCodecError {
  /// Underlying TCP stream I/O failure.
  Io(io::Error),
  /// A frame could not be encoded or decoded.
  Wire(WireError),
}

impl fmt::Display for FrameCodecError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | FrameCodecError::Io(err) => write!(f, "frame codec io error: {err}"),
      | FrameCodecError::Wire(err) => write!(f, "frame codec wire error: {err}"),
    }
  }
}

impl std::error::Error for FrameCodecError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      | FrameCodecError::Io(err) => Some(err),
      | FrameCodecError::Wire(err) => Some(err),
    }
  }
}

impl From<io::Error> for FrameCodecError {
  fn from(err: io::Error) -> Self {
    Self::Io(err)
  }
}

impl From<WireError> for FrameCodecError {
  fn from(err: WireError) -> Self {
    Self::Wire(err)
  }
}
