//! Decoded wire frame paired with its original bytes.

use bytes::Bytes;

use super::wire_frame::WireFrame;

/// A decoded [`WireFrame`] together with the exact bytes read from the socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedWireFrame {
  frame: WireFrame,
  bytes: Bytes,
}

impl DecodedWireFrame {
  pub(crate) const fn new(frame: WireFrame, bytes: Bytes) -> Self {
    Self { frame, bytes }
  }

  /// Returns the decoded frame.
  #[must_use]
  pub const fn frame(&self) -> &WireFrame {
    &self.frame
  }

  /// Returns the original encoded bytes.
  #[must_use]
  pub const fn bytes(&self) -> &Bytes {
    &self.bytes
  }

  /// Consumes this value and returns the decoded frame plus original bytes.
  #[must_use]
  pub fn into_parts(self) -> (WireFrame, Bytes) {
    (self.frame, self.bytes)
  }
}
