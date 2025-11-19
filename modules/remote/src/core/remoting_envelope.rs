//! Serialized outbound frame metadata used by transports.

use alloc::{string::String, vec::Vec};
use core::convert::TryInto;

use fraktor_actor_rs::core::{
  actor_prim::actor_path::{ActorPath, ActorPathParser},
  event_stream::CorrelationId,
  serialization::SerializedMessage,
};

use crate::core::{outbound_priority::OutboundPriority, remote_node_id::RemoteNodeId, wire_error::WireError};

/// Fully serialized outbound message ready for transport framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingEnvelope {
  recipient:      ActorPath,
  remote_node:    RemoteNodeId,
  reply_to:       Option<ActorPath>,
  serialized:     SerializedMessage,
  correlation_id: CorrelationId,
  priority:       OutboundPriority,
}

impl RemotingEnvelope {
  /// Creates a new envelope with the provided components.
  #[must_use]
  pub const fn new(
    recipient: ActorPath,
    remote_node: RemoteNodeId,
    reply_to: Option<ActorPath>,
    serialized: SerializedMessage,
    correlation_id: CorrelationId,
    priority: OutboundPriority,
  ) -> Self {
    Self { recipient, remote_node, reply_to, serialized, correlation_id, priority }
  }

  /// Returns the fully qualified recipient path.
  #[must_use]
  pub fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the remote node metadata resolved during the handshake.
  #[must_use]
  pub fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns the optional reply target path.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorPath> {
    self.reply_to.as_ref()
  }

  /// Returns the serialized payload.
  #[must_use]
  pub fn serialized_message(&self) -> &SerializedMessage {
    &self.serialized
  }

  /// Returns the correlation identifier shared with transport diagnostics.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Returns the logical priority of the message.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  /// Returns `true` when the envelope represents a system message.
  #[must_use]
  pub const fn is_system(&self) -> bool {
    self.priority.is_system()
  }

  /// Encodes the envelope into a binary payload consumed by transports.
  #[must_use]
  pub fn encode_frame(&self) -> Vec<u8> {
    const VERSION: u8 = 1;
    const KIND_MESSAGE: u8 = 0x10;
    let mut buffer = Vec::new();
    buffer.push(VERSION);
    buffer.push(KIND_MESSAGE);
    buffer.push(self.priority.to_wire());
    write_string(&mut buffer, &self.recipient.to_canonical_uri());
    if let Some(reply_to) = self.reply_to.as_ref() {
      buffer.push(1);
      write_string(&mut buffer, &reply_to.to_canonical_uri());
    } else {
      buffer.push(0);
    }
    write_string(&mut buffer, self.remote_node.system());
    write_string(&mut buffer, self.remote_node.host());
    if let Some(port) = self.remote_node.port() {
      buffer.push(1);
      buffer.extend_from_slice(&port.to_le_bytes());
    } else {
      buffer.push(0);
    }
    buffer.extend_from_slice(&self.remote_node.uid().to_le_bytes());
    let serialized = self.serialized.encode();
    buffer.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
    buffer.extend_from_slice(&serialized);
    buffer
  }

  /// Restores an envelope from a binary payload.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the payload is malformed.
  pub fn decode_frame(bytes: &[u8], correlation_id: CorrelationId) -> Result<Self, WireError> {
    const VERSION: u8 = 1;
    const KIND_MESSAGE: u8 = 0x10;
    if bytes.len() < 3 {
      return Err(WireError::InvalidFormat);
    }
    if bytes[0] != VERSION || bytes[1] != KIND_MESSAGE {
      return Err(WireError::InvalidFormat);
    }
    let priority = OutboundPriority::from_wire(bytes[2]).ok_or(WireError::InvalidFormat)?;
    let mut cursor = 3;
    let recipient = ActorPathParser::parse(&read_string(bytes, &mut cursor)?)?;

    let reply_to = if read_bool(bytes, &mut cursor)? {
      Some(ActorPathParser::parse(&read_string(bytes, &mut cursor)?)?)
    } else {
      None
    };

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
    cursor += 8;
    if bytes.len() < cursor + 4 {
      return Err(WireError::InvalidFormat);
    }
    let payload_len =
      u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
    cursor += 4;
    if bytes.len() < cursor + payload_len {
      return Err(WireError::InvalidFormat);
    }
    let payload = &bytes[cursor..cursor + payload_len];
    let serialized = SerializedMessage::decode(payload)?;
    let remote_node = RemoteNodeId::new(system_name, host, port, uid);
    Ok(Self::new(recipient, remote_node, reply_to, serialized, correlation_id, priority))
  }
}

fn write_string(buffer: &mut Vec<u8>, value: &str) {
  let bytes = value.as_bytes();
  buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
  buffer.extend_from_slice(bytes);
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, WireError> {
  if bytes.len() < *cursor + 4 {
    return Err(WireError::InvalidFormat);
  }
  let len = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
  *cursor += 4;
  if bytes.len() < *cursor + len {
    return Err(WireError::InvalidFormat);
  }
  let slice = &bytes[*cursor..*cursor + len];
  *cursor += len;
  Ok(String::from_utf8(slice.to_vec())?)
}

fn read_bool(bytes: &[u8], cursor: &mut usize) -> Result<bool, WireError> {
  if bytes.len() <= *cursor {
    return Err(WireError::InvalidFormat);
  }
  let value = bytes[*cursor];
  *cursor += 1;
  match value {
    | 0 => Ok(false),
    | 1 => Ok(true),
    | _ => Err(WireError::InvalidFormat),
  }
}
