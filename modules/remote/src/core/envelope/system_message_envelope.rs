//! Acked-delivery wire envelope used for system messages.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::convert::TryInto;

use fraktor_actor_rs::core::{
  actor::actor_path::{ActorPath, ActorPathParser},
  event::stream::CorrelationId,
  serialization::SerializedMessage,
};

use super::{priority::OutboundPriority, remoting::RemotingEnvelope};
use crate::core::{remote_node_id::RemoteNodeId, wire_error::WireError, wire_format};

const VERSION: u8 = 1;
/// Wire kind used for system-message frames.
pub const SYSTEM_MESSAGE_FRAME_KIND: u8 = 0x11;

/// System message payload with sequence metadata required for acked delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemMessageEnvelope {
  recipient:      ActorPath,
  remote_node:    RemoteNodeId,
  sender:         Option<ActorPath>,
  serialized:     SerializedMessage,
  correlation_id: CorrelationId,
  sequence_no:    u64,
  ack_reply_to:   RemoteNodeId,
}

impl SystemMessageEnvelope {
  /// Creates a new system-message envelope.
  #[must_use]
  pub const fn new(
    recipient: ActorPath,
    remote_node: RemoteNodeId,
    sender: Option<ActorPath>,
    serialized: SerializedMessage,
    correlation_id: CorrelationId,
    sequence_no: u64,
    ack_reply_to: RemoteNodeId,
  ) -> Self {
    Self { recipient, remote_node, sender, serialized, correlation_id, sequence_no, ack_reply_to }
  }

  /// Wraps a remoting envelope as a system-message envelope.
  #[must_use]
  pub fn from_remoting_envelope(envelope: RemotingEnvelope, sequence_no: u64, ack_reply_to: RemoteNodeId) -> Self {
    Self::new(
      envelope.recipient().clone(),
      envelope.remote_node().clone(),
      envelope.sender().cloned(),
      envelope.serialized_message().clone(),
      envelope.correlation_id(),
      sequence_no,
      ack_reply_to,
    )
  }

  /// Unwraps into the regular remoting envelope representation.
  #[must_use]
  pub fn into_remoting_envelope(self) -> RemotingEnvelope {
    RemotingEnvelope::new(
      self.recipient,
      self.remote_node,
      self.sender,
      self.serialized,
      self.correlation_id,
      OutboundPriority::System,
    )
  }

  /// Returns the recipient actor path.
  #[must_use]
  pub fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the remote node metadata.
  #[must_use]
  pub const fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns the sender actor path when present.
  #[must_use]
  pub fn sender(&self) -> Option<&ActorPath> {
    self.sender.as_ref()
  }

  /// Returns the serialized message payload.
  #[must_use]
  pub const fn serialized_message(&self) -> &SerializedMessage {
    &self.serialized
  }

  /// Returns the correlation identifier.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Returns the sequence number assigned for acked delivery.
  #[must_use]
  pub const fn sequence_no(&self) -> u64 {
    self.sequence_no
  }

  /// Returns the address metadata where ACK/NACK should be sent.
  #[must_use]
  pub const fn ack_reply_to(&self) -> &RemoteNodeId {
    &self.ack_reply_to
  }

  /// Encodes the envelope into a system-message transport frame.
  #[must_use]
  pub fn encode_frame(&self) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.push(VERSION);
    buffer.push(SYSTEM_MESSAGE_FRAME_KIND);
    wire_format::write_string(&mut buffer, &self.recipient.to_canonical_uri());
    if let Some(sender) = self.sender() {
      buffer.push(1);
      wire_format::write_string(&mut buffer, &sender.to_canonical_uri());
    } else {
      buffer.push(0);
    }
    write_node(&mut buffer, &self.remote_node);
    buffer.extend_from_slice(&self.sequence_no.to_le_bytes());
    write_node(&mut buffer, &self.ack_reply_to);
    let serialized = self.serialized.encode();
    buffer.extend_from_slice(&(serialized.len() as u32).to_le_bytes());
    buffer.extend_from_slice(&serialized);
    buffer
  }

  /// Decodes a system-message envelope from the transport frame bytes.
  ///
  /// # Errors
  ///
  /// Returns [`WireError`] when the frame is malformed.
  pub fn decode_frame(bytes: &[u8], correlation_id: CorrelationId) -> Result<Self, WireError> {
    if bytes.len() < 3 || bytes[0] != VERSION || bytes[1] != SYSTEM_MESSAGE_FRAME_KIND {
      return Err(WireError::InvalidFormat);
    }
    let mut cursor = 2;
    let recipient = ActorPathParser::parse(&wire_format::read_string(bytes, &mut cursor)?)?;
    let sender = if wire_format::read_bool(bytes, &mut cursor)? {
      Some(ActorPathParser::parse(&wire_format::read_string(bytes, &mut cursor)?)?)
    } else {
      None
    };
    let remote_node = read_node(bytes, &mut cursor)?;
    let sequence_no = read_u64(bytes, &mut cursor)?;
    let ack_reply_to = read_node(bytes, &mut cursor)?;
    if bytes.len() < cursor + 4 {
      return Err(WireError::InvalidFormat);
    }
    let payload_len =
      u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
    cursor += 4;
    if bytes.len() < cursor + payload_len {
      return Err(WireError::InvalidFormat);
    }
    let serialized = SerializedMessage::decode(&bytes[cursor..cursor + payload_len])?;
    Ok(Self::new(recipient, remote_node, sender, serialized, correlation_id, sequence_no, ack_reply_to))
  }
}

fn write_node(buffer: &mut Vec<u8>, node: &RemoteNodeId) {
  wire_format::write_string(buffer, node.system());
  wire_format::write_string(buffer, node.host());
  if let Some(port) = node.port() {
    buffer.push(1);
    buffer.extend_from_slice(&port.to_le_bytes());
  } else {
    buffer.push(0);
  }
  buffer.extend_from_slice(&node.uid().to_le_bytes());
}

fn read_node(bytes: &[u8], cursor: &mut usize) -> Result<RemoteNodeId, WireError> {
  let system_name = wire_format::read_string(bytes, cursor)?;
  let host = wire_format::read_string(bytes, cursor)?;
  let port = if wire_format::read_bool(bytes, cursor)? {
    if bytes.len() < *cursor + 2 {
      return Err(WireError::InvalidFormat);
    }
    let port = u16::from_le_bytes(bytes[*cursor..*cursor + 2].try_into().map_err(|_| WireError::InvalidFormat)?);
    *cursor += 2;
    Some(port)
  } else {
    None
  };
  let uid = read_u64(bytes, cursor)?;
  Ok(RemoteNodeId::new(system_name, host, port, uid))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, WireError> {
  if bytes.len() < *cursor + 8 {
    return Err(WireError::InvalidFormat);
  }
  let value = u64::from_le_bytes(bytes[*cursor..*cursor + 8].try_into().map_err(|_| WireError::InvalidFormat)?);
  *cursor += 8;
  Ok(value)
}
