//! Error type produced by [`crate::tcp_transport::WireFrameCodec`].

use std::{
  error::Error,
  fmt::{Display, Formatter, Result as FmtResult},
  io::Error as IoError,
};

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
  Io(IoError),

  /// A frame could not be encoded or decoded.
  Wire(WireError),
}

impl Display for FrameCodecError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | FrameCodecError::Io(err) => write!(f, "frame codec io error: {err}"),
      | FrameCodecError::Wire(err) => write!(f, "frame codec wire error: {err}"),
    }
  }
}

impl Error for FrameCodecError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      | FrameCodecError::Io(err) => Some(err),
      | FrameCodecError::Wire(err) => Some(err),
    }
  }
}

impl From<IoError> for FrameCodecError {
  fn from(err: IoError) -> Self {
    Self::Io(err)
  }
}

impl From<WireError> for FrameCodecError {
  fn from(err: WireError) -> Self {
    Self::Wire(err)
  }
}
