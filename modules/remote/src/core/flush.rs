//! Control frame requesting remote-side drain acknowledgement.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::wire_error::WireError;

/// Wire kind used for [`Flush`] control frames.
pub const FLUSH_FRAME_KIND: u8 = 0x20;
const VERSION: u8 = 1;

/// Control message requesting the peer to acknowledge drained pending acknowledgements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flush;

impl Flush {
  /// Creates a new flush request.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Encodes the flush frame.
  #[must_use]
  pub fn encode_frame(&self) -> Vec<u8> {
    Vec::from([VERSION, FLUSH_FRAME_KIND])
  }

  /// Decodes a flush frame from bytes.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  pub fn decode_frame(bytes: &[u8]) -> Result<Self, WireError> {
    if bytes.len() != 2 || bytes[0] != VERSION || bytes[1] != FLUSH_FRAME_KIND {
      return Err(WireError::InvalidFormat);
    }
    Ok(Self)
  }
}

impl Flush {
  /// Returns the wire frame kind associated with the control message.
  #[must_use]
  pub const fn frame_kind(&self) -> u8 {
    FLUSH_FRAME_KIND
  }
}
