//! Codec for [`ControlPdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  codec::Codec,
  control_pdu::ControlPdu,
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
    }
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<ControlPdu, WireError> {
    let _ = read_frame_header(buf, KIND_CONTROL)?;
    if buf.remaining() < 1 {
      return Err(WireError::Truncated);
    }
    let subkind = buf.get_u8();
    let authority = decode_string(buf)?;
    let reason = decode_option_string(buf)?;
    match subkind {
      | SUBKIND_HEARTBEAT => Ok(ControlPdu::Heartbeat { authority }),
      | SUBKIND_QUARANTINE => Ok(ControlPdu::Quarantine { authority, reason }),
      | SUBKIND_SHUTDOWN => Ok(ControlPdu::Shutdown { authority }),
      | _ => Err(WireError::InvalidFormat),
    }
  }
}
