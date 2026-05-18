//! Codec for [`ControlPdu`].

#[cfg(test)]
#[path = "control_codec_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::num::TryFromIntError;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  codec::Codec,
  compression_table_entry::CompressionTableEntry,
  compression_table_kind::CompressionTableKind,
  control_pdu::ControlPdu,
  flush_scope::FlushScope,
  frame_header::KIND_CONTROL,
  primitives::{
    begin_frame, decode_option_string, decode_string, encode_option_string, encode_string, patch_frame_length,
    read_frame_header,
  },
  wire_error::WireError,
};

const SUBKIND_HEARTBEAT: u8 = 0x00;
const SUBKIND_QUARANTINE: u8 = 0x01;
const SUBKIND_SHUTDOWN: u8 = 0x02;
const SUBKIND_HEARTBEAT_RESPONSE: u8 = 0x03;
const SUBKIND_FLUSH_REQUEST: u8 = 0x04;
const SUBKIND_FLUSH_ACK: u8 = 0x05;
const SUBKIND_COMPRESSION_ADVERTISEMENT: u8 = 0x06;
const SUBKIND_COMPRESSION_ACK: u8 = 0x07;
const MIN_COMPRESSION_ENTRY_BYTES: usize = 4 + 4;

/// Zero-sized codec for [`ControlPdu`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlCodec;

impl ControlCodec {
  /// Creates a new [`ControlCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Codec<ControlPdu> for ControlCodec {
  fn encode(&self, value: &ControlPdu, buf: &mut BytesMut) -> Result<(), WireError> {
    let len_pos = begin_frame(buf, KIND_CONTROL);
    encode_control_body(value, buf)?;
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
    read_frame_header(buf, KIND_CONTROL)?;
    if buf.remaining() < 1 {
      return Err(WireError::Truncated);
    }
    let subkind = buf.get_u8();
    let authority = decode_string(buf)?;
    let reason = decode_option_string(buf)?;
    decode_control_body(subkind, authority, reason, buf)
  }
}

fn encode_control_body(value: &ControlPdu, buf: &mut BytesMut) -> Result<(), WireError> {
  match value {
    | ControlPdu::Heartbeat { authority } => encode_authority_control(SUBKIND_HEARTBEAT, authority, None, buf),
    | ControlPdu::HeartbeatResponse { authority, uid } => {
      encode_authority_control(SUBKIND_HEARTBEAT_RESPONSE, authority, None, buf)?;
      buf.put_u64(*uid);
      Ok(())
    },
    | ControlPdu::Quarantine { authority, reason } => {
      encode_authority_control(SUBKIND_QUARANTINE, authority, reason.as_deref(), buf)
    },
    | ControlPdu::Shutdown { authority } => encode_authority_control(SUBKIND_SHUTDOWN, authority, None, buf),
    | ControlPdu::FlushRequest { authority, flush_id, scope, lane_id, expected_acks } => {
      encode_authority_control(SUBKIND_FLUSH_REQUEST, authority, None, buf)?;
      buf.put_u64(*flush_id);
      buf.put_u8(scope.to_wire());
      buf.put_u32(*lane_id);
      buf.put_u32(*expected_acks);
      Ok(())
    },
    | ControlPdu::FlushAck { authority, flush_id, lane_id, expected_acks } => {
      encode_authority_control(SUBKIND_FLUSH_ACK, authority, None, buf)?;
      buf.put_u64(*flush_id);
      buf.put_u32(*lane_id);
      buf.put_u32(*expected_acks);
      Ok(())
    },
    | ControlPdu::CompressionAdvertisement { authority, table_kind, generation, entries } => {
      encode_compression_advertisement(authority, *table_kind, *generation, entries, buf)
    },
    | ControlPdu::CompressionAck { authority, table_kind, generation } => {
      encode_authority_control(SUBKIND_COMPRESSION_ACK, authority, None, buf)?;
      buf.put_u8(table_kind.to_wire());
      buf.put_u64(*generation);
      Ok(())
    },
  }
}

fn encode_authority_control(
  subkind: u8,
  authority: &str,
  reason: Option<&str>,
  buf: &mut BytesMut,
) -> Result<(), WireError> {
  buf.put_u8(subkind);
  encode_string(authority, buf)?;
  encode_option_string(reason, buf)
}

fn encode_compression_advertisement(
  authority: &str,
  table_kind: CompressionTableKind,
  generation: u64,
  entries: &[CompressionTableEntry],
  buf: &mut BytesMut,
) -> Result<(), WireError> {
  encode_authority_control(SUBKIND_COMPRESSION_ADVERTISEMENT, authority, None, buf)?;
  buf.put_u8(table_kind.to_wire());
  buf.put_u64(generation);
  encode_compression_entries(entries, buf)
}

fn decode_control_body(
  subkind: u8,
  authority: String,
  reason: Option<String>,
  buf: &mut Bytes,
) -> Result<ControlPdu, WireError> {
  match subkind {
    | SUBKIND_HEARTBEAT => Ok(ControlPdu::Heartbeat { authority }),
    | SUBKIND_HEARTBEAT_RESPONSE => decode_heartbeat_response(authority, buf),
    | SUBKIND_QUARANTINE => Ok(ControlPdu::Quarantine { authority, reason }),
    | SUBKIND_SHUTDOWN => Ok(ControlPdu::Shutdown { authority }),
    | SUBKIND_FLUSH_REQUEST => decode_flush_request(authority, buf),
    | SUBKIND_FLUSH_ACK => decode_flush_ack(authority, buf),
    | SUBKIND_COMPRESSION_ADVERTISEMENT => decode_compression_advertisement(authority, reason.as_deref(), buf),
    | SUBKIND_COMPRESSION_ACK => decode_compression_ack(authority, reason.as_deref(), buf),
    | _ => Err(WireError::InvalidFormat),
  }
}

fn decode_heartbeat_response(authority: String, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
  ensure_remaining(buf, 8)?;
  Ok(ControlPdu::HeartbeatResponse { authority, uid: buf.get_u64() })
}

fn decode_flush_request(authority: String, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
  ensure_remaining(buf, 17)?;
  let flush_id = buf.get_u64();
  let scope = FlushScope::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
  let lane_id = buf.get_u32();
  let expected_acks = buf.get_u32();
  Ok(ControlPdu::FlushRequest { authority, flush_id, scope, lane_id, expected_acks })
}

fn decode_flush_ack(authority: String, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
  ensure_remaining(buf, 16)?;
  let flush_id = buf.get_u64();
  let lane_id = buf.get_u32();
  let expected_acks = buf.get_u32();
  Ok(ControlPdu::FlushAck { authority, flush_id, lane_id, expected_acks })
}

fn decode_compression_advertisement(
  authority: String,
  reason: Option<&str>,
  buf: &mut Bytes,
) -> Result<ControlPdu, WireError> {
  ensure_no_reason(reason)?;
  ensure_remaining(buf, 13)?;
  let table_kind = CompressionTableKind::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
  let generation = buf.get_u64();
  let entries = decode_compression_entries(buf)?;
  Ok(ControlPdu::CompressionAdvertisement { authority, table_kind, generation, entries })
}

fn decode_compression_ack(authority: String, reason: Option<&str>, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
  ensure_no_reason(reason)?;
  ensure_remaining(buf, 9)?;
  let table_kind = CompressionTableKind::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
  let generation = buf.get_u64();
  Ok(ControlPdu::CompressionAck { authority, table_kind, generation })
}

const fn ensure_no_reason(reason: Option<&str>) -> Result<(), WireError> {
  if reason.is_some() {
    return Err(WireError::InvalidFormat);
  }
  Ok(())
}

fn ensure_remaining(buf: &Bytes, len: usize) -> Result<(), WireError> {
  if buf.remaining() < len {
    return Err(WireError::Truncated);
  }
  Ok(())
}

fn encode_compression_entries(entries: &[CompressionTableEntry], buf: &mut BytesMut) -> Result<(), WireError> {
  buf.put_u32(compression_entry_count(entries.len())?);
  for entry in entries {
    buf.put_u32(entry.id());
    encode_string(entry.literal(), buf)?;
  }
  Ok(())
}

fn compression_entry_count(entries_len: usize) -> Result<u32, WireError> {
  u32::try_from(entries_len).map_err(|_err: TryFromIntError| WireError::InvalidFormat)
}

fn decode_compression_entries(buf: &mut Bytes) -> Result<Vec<CompressionTableEntry>, WireError> {
  let entry_count = buf.get_u32() as usize;
  if entry_count > buf.remaining() / MIN_COMPRESSION_ENTRY_BYTES {
    return Err(WireError::Truncated);
  }
  let mut entries = Vec::with_capacity(entry_count);
  for _ in 0..entry_count {
    let id = buf.get_u32();
    let literal = decode_string(buf)?;
    entries.push(CompressionTableEntry::new(id, literal));
  }
  Ok(entries)
}
