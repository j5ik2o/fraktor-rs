//! Heartbeat probe sent between remote watcher daemons.

use alloc::string::String;
#[cfg(feature = "tokio-transport")]
use alloc::vec::Vec;

#[cfg(feature = "tokio-transport")]
use crate::core::wire_error::WireError;

#[cfg(feature = "tokio-transport")]
const VERSION: u8 = 1;
/// Wire kind used for heartbeat probe frames.
#[cfg(feature = "tokio-transport")]
pub(crate) const HEARTBEAT_FRAME_KIND: u8 = 0x22;

/// Heartbeat probe identifying the source authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Heartbeat {
  pub(crate) authority: String,
}

impl Heartbeat {
  /// Creates a heartbeat probe for the authority.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn new(authority: impl Into<String>) -> Self {
    Self { authority: authority.into() }
  }

  /// Returns the heartbeat source authority.
  #[must_use]
  pub(crate) fn authority(&self) -> &str {
    &self.authority
  }

  /// Encodes the heartbeat probe frame.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn encode_frame(&self) -> Vec<u8> {
    Vec::from([VERSION, HEARTBEAT_FRAME_KIND])
  }

  /// Decodes a heartbeat probe frame using the sender authority from transport metadata.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn decode_frame(bytes: &[u8], authority: impl Into<String>) -> Result<Self, WireError> {
    if bytes.len() != 2 || bytes[0] != VERSION || bytes[1] != HEARTBEAT_FRAME_KIND {
      return Err(WireError::InvalidFormat);
    }
    Ok(Self::new(authority))
  }
}

#[cfg(feature = "tokio-transport")]
impl Heartbeat {
  /// Returns the wire frame kind associated with the control message.
  #[must_use]
  pub(crate) const fn frame_kind(&self) -> u8 {
    HEARTBEAT_FRAME_KIND
  }
}
