#[cfg(test)]
#[path = "stream_ref_protocol_serializer_test.rs"]
mod tests;

use alloc::{
  borrow::Cow,
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  any::{Any, TypeId},
  convert::TryInto,
  num::NonZeroU64,
};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializedMessage, Serializer, SerializerId, SerializerWithStringManifest,
};

use super::{
  StreamRefAck, StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
  StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSinkRefPayload, StreamRefSourceRefPayload,
};

/// Stable serializer identifier for StreamRef protocol payloads.
pub const STREAM_REF_PROTOCOL_SERIALIZER_ID: SerializerId = SerializerId::from_raw(41);
/// Stable serializer name used by [`StreamRefProtocolSerializationSetup`].
///
/// [`StreamRefProtocolSerializationSetup`]: super::StreamRefProtocolSerializationSetup
pub const STREAM_REF_PROTOCOL_SERIALIZER_NAME: &str = "stream-ref-protocol";
/// Manifest for [`StreamRefSequencedOnNext`].
pub const SEQUENCED_ON_NEXT_MANIFEST: &str = "A";
/// Manifest for [`StreamRefCumulativeDemand`].
pub const CUMULATIVE_DEMAND_MANIFEST: &str = "B";
/// Manifest for [`StreamRefRemoteStreamFailure`].
pub const REMOTE_STREAM_FAILURE_MANIFEST: &str = "C";
/// Manifest for [`StreamRefRemoteStreamCompleted`].
pub const REMOTE_STREAM_COMPLETED_MANIFEST: &str = "D";
/// Manifest for [`StreamRefSourceRefPayload`].
pub const SOURCE_REF_MANIFEST: &str = "E";
/// Manifest for [`StreamRefSinkRefPayload`].
pub const SINK_REF_MANIFEST: &str = "F";
/// Manifest for [`StreamRefOnSubscribeHandshake`].
pub const ON_SUBSCRIBE_HANDSHAKE_MANIFEST: &str = "G";
/// Manifest for [`StreamRefAck`].
pub const ACK_MANIFEST: &str = "H";

const LEN_PREFIX_BYTES: usize = core::mem::size_of::<u32>();
const U64_BYTES: usize = core::mem::size_of::<u64>();

/// Serializer for low-level StreamRef protocol payloads.
pub struct StreamRefProtocolSerializer {
  id: SerializerId,
}

impl StreamRefProtocolSerializer {
  /// Creates a serializer with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }

  fn encode_sequenced_on_next(message: &StreamRefSequencedOnNext) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    write_u64(&mut buffer, message.seq_nr());
    let payload = message.payload().encode();
    write_len_prefixed_bytes(&mut buffer, &payload)?;
    Ok(buffer)
  }

  fn decode_sequenced_on_next(bytes: &[u8]) -> Result<StreamRefSequencedOnNext, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let seq_nr = cursor.read_u64()?;
    let payload = SerializedMessage::decode(cursor.read_len_prefixed_bytes()?)?;
    cursor.ensure_finished()?;
    Ok(StreamRefSequencedOnNext::new(seq_nr, payload))
  }

  fn encode_cumulative_demand(message: &StreamRefCumulativeDemand) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(U64_BYTES * 2);
    write_u64(&mut buffer, message.seq_nr());
    write_u64(&mut buffer, message.demand().get());
    buffer
  }

  fn decode_cumulative_demand(bytes: &[u8]) -> Result<StreamRefCumulativeDemand, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let seq_nr = cursor.read_u64()?;
    let raw_demand = cursor.read_u64()?;
    cursor.ensure_finished()?;
    let demand = NonZeroU64::new(raw_demand).ok_or(SerializationError::InvalidFormat)?;
    Ok(StreamRefCumulativeDemand::new(seq_nr, demand))
  }

  fn encode_on_subscribe_handshake(message: &StreamRefOnSubscribeHandshake) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    write_len_prefixed_bytes(&mut buffer, message.target_ref_path().as_bytes())?;
    Ok(buffer)
  }

  fn decode_on_subscribe_handshake(bytes: &[u8]) -> Result<StreamRefOnSubscribeHandshake, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let target_ref_path = cursor.read_string()?;
    cursor.ensure_finished()?;
    Ok(StreamRefOnSubscribeHandshake::new(target_ref_path))
  }

  fn encode_remote_stream_completed(message: StreamRefRemoteStreamCompleted) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(U64_BYTES);
    write_u64(&mut buffer, message.seq_nr());
    buffer
  }

  fn decode_remote_stream_completed(bytes: &[u8]) -> Result<StreamRefRemoteStreamCompleted, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let seq_nr = cursor.read_u64()?;
    cursor.ensure_finished()?;
    Ok(StreamRefRemoteStreamCompleted::new(seq_nr))
  }

  fn encode_remote_stream_failure(message: &StreamRefRemoteStreamFailure) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    write_len_prefixed_bytes(&mut buffer, message.message().as_bytes())?;
    Ok(buffer)
  }

  fn decode_remote_stream_failure(bytes: &[u8]) -> Result<StreamRefRemoteStreamFailure, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let message = cursor.read_string()?;
    cursor.ensure_finished()?;
    Ok(StreamRefRemoteStreamFailure::new(message))
  }

  const fn decode_ack(bytes: &[u8]) -> Result<StreamRefAck, SerializationError> {
    if bytes.is_empty() { Ok(StreamRefAck) } else { Err(SerializationError::InvalidFormat) }
  }

  fn encode_source_ref_payload(message: &StreamRefSourceRefPayload) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    write_len_prefixed_bytes(&mut buffer, message.actor_path().as_bytes())?;
    Ok(buffer)
  }

  fn decode_source_ref_payload(bytes: &[u8]) -> Result<StreamRefSourceRefPayload, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let actor_path = cursor.read_string()?;
    cursor.ensure_finished()?;
    Ok(StreamRefSourceRefPayload::new(actor_path))
  }

  fn encode_sink_ref_payload(message: &StreamRefSinkRefPayload) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    write_len_prefixed_bytes(&mut buffer, message.actor_path().as_bytes())?;
    Ok(buffer)
  }

  fn decode_sink_ref_payload(bytes: &[u8]) -> Result<StreamRefSinkRefPayload, SerializationError> {
    let mut cursor = Cursor::new(bytes);
    let actor_path = cursor.read_string()?;
    cursor.ensure_finished()?;
    Ok(StreamRefSinkRefPayload::new(actor_path))
  }
}

impl Serializer for StreamRefProtocolSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    if let Some(message) = message.downcast_ref::<StreamRefSequencedOnNext>() {
      return Self::encode_sequenced_on_next(message);
    }
    if let Some(message) = message.downcast_ref::<StreamRefCumulativeDemand>() {
      return Ok(Self::encode_cumulative_demand(message));
    }
    if let Some(message) = message.downcast_ref::<StreamRefOnSubscribeHandshake>() {
      return Self::encode_on_subscribe_handshake(message);
    }
    if let Some(message) = message.downcast_ref::<StreamRefRemoteStreamCompleted>() {
      return Ok(Self::encode_remote_stream_completed(*message));
    }
    if let Some(message) = message.downcast_ref::<StreamRefRemoteStreamFailure>() {
      return Self::encode_remote_stream_failure(message);
    }
    if message.downcast_ref::<StreamRefAck>().is_some() {
      return Ok(Vec::new());
    }
    if let Some(message) = message.downcast_ref::<StreamRefSourceRefPayload>() {
      return Self::encode_source_ref_payload(message);
    }
    if let Some(message) = message.downcast_ref::<StreamRefSinkRefPayload>() {
      return Self::encode_sink_ref_payload(message);
    }
    Err(SerializationError::InvalidFormat)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let Some(type_id) = type_hint else {
      return Err(SerializationError::InvalidFormat);
    };
    if type_id == TypeId::of::<StreamRefSequencedOnNext>() {
      return Ok(Box::new(Self::decode_sequenced_on_next(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefCumulativeDemand>() {
      return Ok(Box::new(Self::decode_cumulative_demand(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefOnSubscribeHandshake>() {
      return Ok(Box::new(Self::decode_on_subscribe_handshake(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefRemoteStreamCompleted>() {
      return Ok(Box::new(Self::decode_remote_stream_completed(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefRemoteStreamFailure>() {
      return Ok(Box::new(Self::decode_remote_stream_failure(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefAck>() {
      return Ok(Box::new(Self::decode_ack(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefSourceRefPayload>() {
      return Ok(Box::new(Self::decode_source_ref_payload(bytes)?));
    }
    if type_id == TypeId::of::<StreamRefSinkRefPayload>() {
      return Ok(Box::new(Self::decode_sink_ref_payload(bytes)?));
    }
    Err(SerializationError::InvalidFormat)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for StreamRefProtocolSerializer {
  fn manifest(&self, message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    if message.downcast_ref::<StreamRefSequencedOnNext>().is_some() {
      return Cow::Borrowed(SEQUENCED_ON_NEXT_MANIFEST);
    }
    if message.downcast_ref::<StreamRefCumulativeDemand>().is_some() {
      return Cow::Borrowed(CUMULATIVE_DEMAND_MANIFEST);
    }
    if message.downcast_ref::<StreamRefRemoteStreamFailure>().is_some() {
      return Cow::Borrowed(REMOTE_STREAM_FAILURE_MANIFEST);
    }
    if message.downcast_ref::<StreamRefRemoteStreamCompleted>().is_some() {
      return Cow::Borrowed(REMOTE_STREAM_COMPLETED_MANIFEST);
    }
    if message.downcast_ref::<StreamRefOnSubscribeHandshake>().is_some() {
      return Cow::Borrowed(ON_SUBSCRIBE_HANDSHAKE_MANIFEST);
    }
    if message.downcast_ref::<StreamRefAck>().is_some() {
      return Cow::Borrowed(ACK_MANIFEST);
    }
    if message.downcast_ref::<StreamRefSourceRefPayload>().is_some() {
      return Cow::Borrowed(SOURCE_REF_MANIFEST);
    }
    if message.downcast_ref::<StreamRefSinkRefPayload>().is_some() {
      return Cow::Borrowed(SINK_REF_MANIFEST);
    }
    Cow::Borrowed("")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    manifest: &str,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    match manifest {
      | SEQUENCED_ON_NEXT_MANIFEST => Ok(Box::new(Self::decode_sequenced_on_next(bytes)?)),
      | CUMULATIVE_DEMAND_MANIFEST => Ok(Box::new(Self::decode_cumulative_demand(bytes)?)),
      | REMOTE_STREAM_FAILURE_MANIFEST => Ok(Box::new(Self::decode_remote_stream_failure(bytes)?)),
      | REMOTE_STREAM_COMPLETED_MANIFEST => Ok(Box::new(Self::decode_remote_stream_completed(bytes)?)),
      | SOURCE_REF_MANIFEST => Ok(Box::new(Self::decode_source_ref_payload(bytes)?)),
      | SINK_REF_MANIFEST => Ok(Box::new(Self::decode_sink_ref_payload(bytes)?)),
      | ON_SUBSCRIBE_HANDSHAKE_MANIFEST => Ok(Box::new(Self::decode_on_subscribe_handshake(bytes)?)),
      | ACK_MANIFEST => Ok(Box::new(Self::decode_ack(bytes)?)),
      | other => Err(SerializationError::UnknownManifest(other.to_string())),
    }
  }
}

fn write_len_prefixed_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SerializationError> {
  write_u32(buffer, bytes.len())?;
  buffer.extend_from_slice(bytes);
  Ok(())
}

fn write_u32(buffer: &mut Vec<u8>, value: usize) -> Result<(), SerializationError> {
  let value = u32::try_from(value).map_err(|_| SerializationError::InvalidFormat)?;
  buffer.extend_from_slice(&value.to_le_bytes());
  Ok(())
}

fn write_u64(buffer: &mut Vec<u8>, value: u64) {
  buffer.extend_from_slice(&value.to_le_bytes());
}

struct Cursor<'a> {
  bytes:  &'a [u8],
  offset: usize,
}

impl<'a> Cursor<'a> {
  const fn new(bytes: &'a [u8]) -> Self {
    Self { bytes, offset: 0 }
  }

  const fn ensure_finished(&self) -> Result<(), SerializationError> {
    if self.offset == self.bytes.len() { Ok(()) } else { Err(SerializationError::InvalidFormat) }
  }

  fn read_u32(&mut self) -> Result<u32, SerializationError> {
    if self.bytes.len().saturating_sub(self.offset) < LEN_PREFIX_BYTES {
      return Err(SerializationError::InvalidFormat);
    }
    let raw = self.bytes[self.offset..self.offset + LEN_PREFIX_BYTES]
      .try_into()
      .map_err(|_| SerializationError::InvalidFormat)?;
    self.offset += LEN_PREFIX_BYTES;
    Ok(u32::from_le_bytes(raw))
  }

  fn read_u64(&mut self) -> Result<u64, SerializationError> {
    if self.bytes.len().saturating_sub(self.offset) < U64_BYTES {
      return Err(SerializationError::InvalidFormat);
    }
    let raw =
      self.bytes[self.offset..self.offset + U64_BYTES].try_into().map_err(|_| SerializationError::InvalidFormat)?;
    self.offset += U64_BYTES;
    Ok(u64::from_le_bytes(raw))
  }

  fn read_len_prefixed_bytes(&mut self) -> Result<&'a [u8], SerializationError> {
    let len = self.read_u32()? as usize;
    if self.bytes.len().saturating_sub(self.offset) < len {
      return Err(SerializationError::InvalidFormat);
    }
    let bytes = &self.bytes[self.offset..self.offset + len];
    self.offset += len;
    Ok(bytes)
  }

  fn read_string(&mut self) -> Result<String, SerializationError> {
    let bytes = self.read_len_prefixed_bytes()?;
    let value = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(String::from(value))
  }
}
