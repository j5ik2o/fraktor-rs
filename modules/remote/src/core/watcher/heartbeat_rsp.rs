//! Heartbeat response exchanged by remote watcher daemons.

use alloc::string::String;
#[cfg(feature = "tokio-transport")]
use alloc::vec::Vec;
#[cfg(feature = "tokio-transport")]
use core::convert::TryInto;

#[cfg(feature = "tokio-transport")]
use crate::core::{control_message::ControlMessage, wire_error::WireError};

#[cfg(feature = "tokio-transport")]
const VERSION: u8 = 1;
/// Wire kind used for heartbeat response frames.
#[cfg(feature = "tokio-transport")]
pub(crate) const HEARTBEAT_RSP_FRAME_KIND: u8 = 0x23;

/// Heartbeat response identifying the responder authority and actor-system uid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeartbeatRsp {
  pub(crate) authority: String,
  pub(crate) uid:       u64,
}

impl HeartbeatRsp {
  /// Creates a heartbeat response payload.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn new(authority: impl Into<String>, uid: u64) -> Self {
    Self { authority: authority.into(), uid }
  }

  /// Returns the responder authority.
  #[must_use]
  pub(crate) fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the responder actor-system uid.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) const fn uid(&self) -> u64 {
    self.uid
  }

  /// Encodes the heartbeat response frame.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn encode_frame(&self) -> Vec<u8> {
    let mut buffer = Vec::from([VERSION, HEARTBEAT_RSP_FRAME_KIND]);
    buffer.extend_from_slice(&self.uid.to_le_bytes());
    buffer
  }

  /// Decodes a heartbeat response frame using the sender authority from transport metadata.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn decode_frame(bytes: &[u8], authority: impl Into<String>) -> Result<Self, WireError> {
    if bytes.len() != 10 || bytes[0] != VERSION || bytes[1] != HEARTBEAT_RSP_FRAME_KIND {
      return Err(WireError::InvalidFormat);
    }
    let uid = u64::from_le_bytes(bytes[2..10].try_into().map_err(|_| WireError::InvalidFormat)?);
    Ok(Self::new(authority, uid))
  }
}

#[cfg(feature = "tokio-transport")]
impl ControlMessage for HeartbeatRsp {
  fn frame_kind(&self) -> u8 {
    HEARTBEAT_RSP_FRAME_KIND
  }
}
