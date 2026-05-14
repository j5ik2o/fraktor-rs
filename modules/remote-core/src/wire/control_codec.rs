//! Codec for [`ControlPdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  codec::Codec,
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
      | _ => Err(WireError::InvalidFormat),
    }
  }
}
