//! Control frame acknowledging a flush request.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::convert::TryInto;

use crate::core::{control_message::ControlMessage, wire_error::WireError};

/// Wire kind used for [`FlushAck`] frames.
pub const FLUSH_ACK_FRAME_KIND: u8 = 0x21;
const VERSION: u8 = 1;

/// Acknowledges a [`crate::core::flush::Flush`] request with the number of pending ACKs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlushAck {
  expected_acks: u32,
}

impl FlushAck {
  /// Creates a new flush acknowledgement payload.
  #[must_use]
  pub const fn new(expected_acks: u32) -> Self {
    Self { expected_acks }
  }

  /// Returns the number of outstanding acknowledgements seen by the responder.
  #[must_use]
  pub const fn expected_acks(&self) -> u32 {
    self.expected_acks
  }

  /// Encodes the flush acknowledgement frame.
  #[must_use]
  pub fn encode_frame(&self) -> Vec<u8> {
    let mut buffer = Vec::from([VERSION, FLUSH_ACK_FRAME_KIND]);
    buffer.extend_from_slice(&self.expected_acks.to_le_bytes());
    buffer
  }

  /// Decodes a flush acknowledgement frame from bytes.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  pub fn decode_frame(bytes: &[u8]) -> Result<Self, WireError> {
    if bytes.len() != 6 || bytes[0] != VERSION || bytes[1] != FLUSH_ACK_FRAME_KIND {
      return Err(WireError::InvalidFormat);
    }
    let expected_acks = u32::from_le_bytes(bytes[2..6].try_into().map_err(|_| WireError::InvalidFormat)?);
    Ok(Self::new(expected_acks))
  }
}

impl ControlMessage for FlushAck {
  fn frame_kind(&self) -> u8 {
    FLUSH_ACK_FRAME_KIND
  }
}
