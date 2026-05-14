//! Codec for [`ControlPdu`].

use alloc::vec::Vec;

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
    match value {
      | ControlPdu::Heartbeat { authority } => {
        buf.put_u8(SUBKIND_HEARTBEAT);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
      },
      | ControlPdu::HeartbeatResponse { authority, uid } => {
        buf.put_u8(SUBKIND_HEARTBEAT_RESPONSE);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
        buf.put_u64(*uid);
      },
      | ControlPdu::Quarantine { authority, reason } => {
        buf.put_u8(SUBKIND_QUARANTINE);
        encode_string(authority, buf)?;
        encode_option_string(reason.as_deref(), buf)?;
      },
      | ControlPdu::Shutdown { authority } => {
        buf.put_u8(SUBKIND_SHUTDOWN);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
      },
      | ControlPdu::FlushRequest { authority, flush_id, scope, lane_id, expected_acks } => {
        buf.put_u8(SUBKIND_FLUSH_REQUEST);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
        buf.put_u64(*flush_id);
        buf.put_u8(scope.to_wire());
        buf.put_u32(*lane_id);
        buf.put_u32(*expected_acks);
      },
      | ControlPdu::FlushAck { authority, flush_id, lane_id, expected_acks } => {
        buf.put_u8(SUBKIND_FLUSH_ACK);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
        buf.put_u64(*flush_id);
        buf.put_u32(*lane_id);
        buf.put_u32(*expected_acks);
      },
      | ControlPdu::CompressionAdvertisement { authority, table_kind, generation, entries } => {
        buf.put_u8(SUBKIND_COMPRESSION_ADVERTISEMENT);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
        buf.put_u8(table_kind.to_wire());
        buf.put_u64(*generation);
        encode_compression_entries(entries, buf)?;
      },
      | ControlPdu::CompressionAck { authority, table_kind, generation } => {
        buf.put_u8(SUBKIND_COMPRESSION_ACK);
        encode_string(authority, buf)?;
        encode_option_string(None, buf)?;
        buf.put_u8(table_kind.to_wire());
        buf.put_u64(*generation);
      },
    }
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
    match subkind {
      | SUBKIND_HEARTBEAT => Ok(ControlPdu::Heartbeat { authority }),
      | SUBKIND_HEARTBEAT_RESPONSE => {
        if buf.remaining() < 8 {
          return Err(WireError::Truncated);
        }
        Ok(ControlPdu::HeartbeatResponse { authority, uid: buf.get_u64() })
      },
      | SUBKIND_QUARANTINE => Ok(ControlPdu::Quarantine { authority, reason }),
      | SUBKIND_SHUTDOWN => Ok(ControlPdu::Shutdown { authority }),
      | SUBKIND_FLUSH_REQUEST => {
        if buf.remaining() < 17 {
          return Err(WireError::Truncated);
        }
        let flush_id = buf.get_u64();
        let scope = FlushScope::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
        let lane_id = buf.get_u32();
        let expected_acks = buf.get_u32();
        Ok(ControlPdu::FlushRequest { authority, flush_id, scope, lane_id, expected_acks })
      },
      | SUBKIND_FLUSH_ACK => {
        if buf.remaining() < 16 {
          return Err(WireError::Truncated);
        }
        let flush_id = buf.get_u64();
        let lane_id = buf.get_u32();
        let expected_acks = buf.get_u32();
        Ok(ControlPdu::FlushAck { authority, flush_id, lane_id, expected_acks })
      },
      | SUBKIND_COMPRESSION_ADVERTISEMENT => {
        if reason.is_some() || buf.remaining() < 13 {
          return Err(WireError::InvalidFormat);
        }
        let table_kind = CompressionTableKind::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
        let generation = buf.get_u64();
        let entries = decode_compression_entries(buf)?;
        Ok(ControlPdu::CompressionAdvertisement { authority, table_kind, generation, entries })
      },
      | SUBKIND_COMPRESSION_ACK => {
        if reason.is_some() || buf.remaining() < 9 {
          return Err(WireError::InvalidFormat);
        }
        let table_kind = CompressionTableKind::from_wire(buf.get_u8()).ok_or(WireError::InvalidFormat)?;
        let generation = buf.get_u64();
        Ok(ControlPdu::CompressionAck { authority, table_kind, generation })
      },
      | _ => Err(WireError::InvalidFormat),
    }
  }
}

fn encode_compression_entries(entries: &[CompressionTableEntry], buf: &mut BytesMut) -> Result<(), WireError> {
  if entries.len() > u32::MAX as usize {
    return Err(WireError::InvalidFormat);
  }
  buf.put_u32(entries.len() as u32);
  for entry in entries {
    buf.put_u32(entry.id());
    encode_string(entry.literal(), buf)?;
  }
  Ok(())
}

fn decode_compression_entries(buf: &mut Bytes) -> Result<Vec<CompressionTableEntry>, WireError> {
  if buf.remaining() < 4 {
    return Err(WireError::Truncated);
  }
  let entry_count = buf.get_u32() as usize;
  let mut entries = Vec::with_capacity(entry_count);
  for _ in 0..entry_count {
    if buf.remaining() < 4 {
      return Err(WireError::Truncated);
    }
    let id = buf.get_u32();
    let literal = decode_string(buf)?;
    entries.push(CompressionTableEntry::new(id, literal));
  }
  Ok(entries)
}
