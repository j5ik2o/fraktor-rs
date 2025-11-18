//! Converts outbound envelopes into serialized remoting frames.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::str;

use fraktor_actor_rs::core::{
  actor_prim::actor_path::{ActorPath, ActorPathFormatter, ActorPathParser, ActorPathParts},
  event_stream::BackpressureSignal,
  messaging::{AnyMessageGeneric, SystemMessage},
  serialization::{SerializationCallScope, SerializationError, SerializationExtensionGeneric, SerializedMessage},
};
use fraktor_utils_rs::core::{collections::queue::QueueError, runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use self::outbound_queue::{EnvelopePriority, OutboundQueue};
use crate::core::endpoint_manager::RemoteNodeId;

pub mod outbound_queue;

/// Envelope emitted by the endpoint writer, ready for transport serialization.
pub struct RemotingEnvelope {
  target:   ActorPathParts,
  remote:   RemoteNodeId,
  payload:  SerializedMessage,
  reply_to: Option<ActorPathParts>,
}

impl RemotingEnvelope {
  /// Returns target actor path parts.
  #[must_use]
  pub fn target(&self) -> &ActorPathParts {
    &self.target
  }

  /// Returns the remote node metadata.
  #[must_use]
  pub fn remote(&self) -> &RemoteNodeId {
    &self.remote
  }

  /// Returns serialized payload bytes.
  #[must_use]
  pub fn payload(&self) -> &SerializedMessage {
    &self.payload
  }

  /// Returns reply-to actor path when available.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorPathParts> {
    self.reply_to.as_ref()
  }

  /// Encodes the envelope into wire bytes.
  pub fn encode(&self) -> Vec<u8> {
    let mut buffer = Vec::new();
    encode_actor_path_parts(&mut buffer, &self.target);
    encode_remote_node(&mut buffer, &self.remote);
    encode_reply_to(&mut buffer, self.reply_to.as_ref());
    let payload_bytes = self.payload.encode();
    write_u32(&mut buffer, payload_bytes.len() as u32);
    buffer.extend_from_slice(&payload_bytes);
    buffer
  }

  /// Decodes the envelope from wire bytes.
  pub fn decode(bytes: &[u8]) -> Result<Self, SerializationError> {
    let mut cursor = FrameCursor::new(bytes);
    let target = decode_actor_path_parts(&mut cursor)?;
    let remote = decode_remote_node(&mut cursor)?;
    let reply_to = if cursor.read_bool()? { Some(decode_actor_path_parts(&mut cursor)?) } else { None };
    let payload_len = cursor.read_u32()? as usize;
    let payload_bytes = cursor.read_bytes(payload_len)?;
    let payload = SerializedMessage::decode(payload_bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(Self { target, remote, payload, reply_to })
  }
}

fn encode_actor_path_parts(buffer: &mut Vec<u8>, parts: &ActorPathParts) {
  let canonical = ActorPathFormatter::format(&ActorPath::from_parts(parts.clone()));
  write_string(buffer, &canonical);
}

fn decode_actor_path_parts(cursor: &mut FrameCursor<'_>) -> Result<ActorPathParts, SerializationError> {
  let canonical = cursor.read_string()?;
  let actor_path = ActorPathParser::parse(&canonical).map_err(|_| SerializationError::InvalidFormat)?;
  Ok(actor_path.parts().clone())
}

fn encode_remote_node(buffer: &mut Vec<u8>, remote: &RemoteNodeId) {
  write_string(buffer, remote.system());
  write_string(buffer, remote.host());
  match remote.port() {
    | Some(port) => {
      write_bool(buffer, true);
      write_u16(buffer, port);
    },
    | None => write_bool(buffer, false),
  }
  write_u64(buffer, remote.uid());
}

fn decode_remote_node(cursor: &mut FrameCursor<'_>) -> Result<RemoteNodeId, SerializationError> {
  let system = cursor.read_string()?;
  let host = cursor.read_string()?;
  let port = if cursor.read_bool()? { Some(cursor.read_u16()?) } else { None };
  let uid = cursor.read_u64()?;
  Ok(RemoteNodeId::new(system, host, port, uid))
}

fn encode_reply_to(buffer: &mut Vec<u8>, reply_to: Option<&ActorPathParts>) {
  match reply_to {
    | Some(parts) => {
      write_bool(buffer, true);
      encode_actor_path_parts(buffer, parts);
    },
    | None => write_bool(buffer, false),
  }
}

fn write_string(buffer: &mut Vec<u8>, value: &str) {
  let bytes = value.as_bytes();
  write_u32(buffer, bytes.len() as u32);
  buffer.extend_from_slice(bytes);
}

fn write_bool(buffer: &mut Vec<u8>, value: bool) {
  buffer.push(u8::from(value));
}

fn write_u16(buffer: &mut Vec<u8>, value: u16) {
  buffer.extend_from_slice(&value.to_be_bytes());
}

fn write_u32(buffer: &mut Vec<u8>, value: u32) {
  buffer.extend_from_slice(&value.to_be_bytes());
}

fn write_u64(buffer: &mut Vec<u8>, value: u64) {
  buffer.extend_from_slice(&value.to_be_bytes());
}

struct FrameCursor<'a> {
  bytes: &'a [u8],
  pos:   usize,
}

impl<'a> FrameCursor<'a> {
  fn new(bytes: &'a [u8]) -> Self {
    Self { bytes, pos: 0 }
  }

  fn read_bool(&mut self) -> Result<bool, SerializationError> {
    Ok(self.read_u8()? != 0)
  }

  fn read_u8(&mut self) -> Result<u8, SerializationError> {
    if self.pos >= self.bytes.len() {
      return Err(SerializationError::InvalidFormat);
    }
    let value = self.bytes[self.pos];
    self.pos += 1;
    Ok(value)
  }

  fn read_u16(&mut self) -> Result<u16, SerializationError> {
    let slice = self.read_bytes_const::<2>()?;
    Ok(u16::from_be_bytes(slice))
  }

  fn read_u32(&mut self) -> Result<u32, SerializationError> {
    let slice = self.read_bytes_const::<4>()?;
    Ok(u32::from_be_bytes(slice))
  }

  fn read_u64(&mut self) -> Result<u64, SerializationError> {
    let slice = self.read_bytes_const::<8>()?;
    Ok(u64::from_be_bytes(slice))
  }

  fn read_string(&mut self) -> Result<String, SerializationError> {
    let len = self.read_u32()? as usize;
    let bytes = self.read_bytes(len)?;
    let value = str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(value.to_string())
  }

  fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], SerializationError> {
    if self.pos.checked_add(len).map_or(true, |end| end > self.bytes.len()) {
      return Err(SerializationError::InvalidFormat);
    }
    let start = self.pos;
    let end = start + len;
    self.pos = end;
    Ok(&self.bytes[start..end])
  }

  fn read_bytes_const<const N: usize>(&mut self) -> Result<[u8; N], SerializationError> {
    let bytes = self.read_bytes(N)?;
    let mut array = [0u8; N];
    array.copy_from_slice(bytes);
    Ok(array)
  }
}

/// Outbound envelope submitted to the writer.
pub struct OutboundEnvelope<TB: RuntimeToolbox + 'static> {
  /// Destination actor path.
  pub target:  ActorPathParts,
  /// Remote node metadata.
  pub remote:  RemoteNodeId,
  /// Message payload.
  pub message: AnyMessageGeneric<TB>,
}

/// Serializes outbound envelopes using the actor serialization extension.
pub struct EndpointWriter<TB: RuntimeToolbox + 'static> {
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
  queue:         OutboundQueue<TB, OutboundEnvelope<TB>>,
}

impl<TB: RuntimeToolbox + 'static> EndpointWriter<TB> {
  /// Creates a writer backed by the provided serialization extension.
  #[must_use]
  pub fn new(serialization: ArcShared<SerializationExtensionGeneric<TB>>) -> Self {
    Self { serialization, queue: OutboundQueue::new() }
  }

  /// Serializes the outbound envelope into a remoting envelope.
  pub fn write(&self, envelope: OutboundEnvelope<TB>) -> Result<RemotingEnvelope, SerializationError> {
    let payload = self.serialization.serialize(envelope.message.payload(), SerializationCallScope::Remote)?;
    let reply_to = envelope.message.reply_to().and_then(|reply| reply.path().map(|path| path.parts().clone()));

    Ok(RemotingEnvelope { target: envelope.target, remote: envelope.remote, payload, reply_to })
  }

  /// Enqueues an envelope for later transmission.
  pub fn enqueue(&mut self, envelope: OutboundEnvelope<TB>) -> Result<(), QueueError<OutboundEnvelope<TB>>> {
    self.queue.push(envelope, |env| {
      if env.message.payload().is::<SystemMessage>() { EnvelopePriority::System } else { EnvelopePriority::User }
    })
  }

  /// Pops the next envelope respecting system priority.
  #[must_use]
  pub fn dequeue(&mut self) -> Result<Option<OutboundEnvelope<TB>>, QueueError<OutboundEnvelope<TB>>> {
    self.queue.pop()
  }

  /// Applies transport backpressure signals to pause/resume user traffic.
  pub fn notify_backpressure(&mut self, signal: BackpressureSignal) {
    match signal {
      | BackpressureSignal::Apply => self.queue.pause_user(),
      | BackpressureSignal::Release => self.queue.resume_user(),
    }
  }
}

#[cfg(test)]
mod tests;
