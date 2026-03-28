//! Binary representation of remoting handshake frames.

use alloc::{string::String, vec::Vec};
use core::convert::TryInto;

use super::kind::HandshakeKind;
use crate::core::wire::{WireError, read_bool, read_string, write_bool, write_string};

/// Payload exchanged when establishing associations.
pub struct HandshakeFrame {
  kind:        HandshakeKind,
  system_name: String,
  host:        String,
  port:        Option<u16>,
  uid:         u64,
}

impl HandshakeFrame {
  /// Creates a new handshake frame descriptor.
  #[must_use]
  pub fn new(
    kind: HandshakeKind,
    system_name: impl Into<String>,
    host: impl Into<String>,
    port: Option<u16>,
    uid: u64,
  ) -> Self {
    Self { kind, system_name: system_name.into(), host: host.into(), port, uid }
  }

  /// Returns the handshake kind.
  #[must_use]
  pub const fn kind(&self) -> HandshakeKind {
    self.kind
  }

  /// Returns the remote system name.
  #[must_use]
  pub fn system_name(&self) -> &str {
    &self.system_name
  }

  /// Returns the remote host.
  #[must_use]
  pub fn host(&self) -> &str {
    &self.host
  }

  /// Returns the remote port.
  #[must_use]
  pub const fn port(&self) -> Option<u16> {
    self.port
  }

  /// Returns the advertised UID.
  #[must_use]
  pub const fn uid(&self) -> u64 {
    self.uid
  }

  /// Encodes the frame into a transport payload.
  #[must_use]
  pub fn encode(&self) -> Vec<u8> {
    const VERSION: u8 = 1;
    let mut buffer = Vec::new();
    buffer.push(VERSION);
    buffer.push(self.kind.to_wire());
    write_string(&mut buffer, &self.system_name);
    write_string(&mut buffer, &self.host);
    write_bool(&mut buffer, self.port.is_some());
    if let Some(port) = self.port {
      buffer.extend_from_slice(&port.to_le_bytes());
    }
    buffer.extend_from_slice(&self.uid.to_le_bytes());
    buffer
  }

  /// Decodes a handshake frame from the provided payload.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
    const VERSION: u8 = 1;
    if bytes.len() < 2 {
      return Err(WireError::InvalidFormat);
    }
    if bytes[0] != VERSION {
      return Err(WireError::InvalidFormat);
    }
    let Some(kind) = HandshakeKind::from_wire(bytes[1]) else {
      return Err(WireError::InvalidFormat);
    };
    let mut cursor = 2;
    let system_name = read_string(bytes, &mut cursor)?;
    let host = read_string(bytes, &mut cursor)?;
    let port = if read_bool(bytes, &mut cursor)? {
      if bytes.len() < cursor + 2 {
        return Err(WireError::InvalidFormat);
      }
      let port = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().map_err(|_| WireError::InvalidFormat)?);
      cursor += 2;
      Some(port)
    } else {
      None
    };
    if bytes.len() < cursor + 8 {
      return Err(WireError::InvalidFormat);
    }
    let uid = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().map_err(|_| WireError::InvalidFormat)?);
    Ok(Self::new(kind, system_name, host, port, uid))
  }
}
