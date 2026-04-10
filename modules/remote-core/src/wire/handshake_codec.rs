//! Codec for [`HandshakePdu`].

use alloc::string::String;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  codec::Codec,
  frame_header::{KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP},
  handshake_pdu::HandshakePdu,
  handshake_req::HandshakeReq,
  handshake_rsp::HandshakeRsp,
  primitives::{begin_frame, decode_string, encode_string, patch_frame_length, peek_frame_kind, read_frame_header},
  wire_error::WireError,
};

/// Zero-sized codec for [`HandshakePdu`].
#[derive(Clone, Copy, Debug, Default)]
pub struct HandshakeCodec;

impl HandshakeCodec {
  /// Creates a new [`HandshakeCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

fn encode_body(
  origin_system: &str,
  origin_host: &str,
  origin_port: u16,
  origin_uid: u64,
  buf: &mut BytesMut,
) -> Result<(), WireError> {
  encode_string(origin_system, buf)?;
  encode_string(origin_host, buf)?;
  buf.put_u16(origin_port);
  buf.put_u64(origin_uid);
  Ok(())
}

fn decode_body(buf: &mut Bytes) -> Result<(String, String, u16, u64), WireError> {
  let origin_system = decode_string(buf)?;
  let origin_host = decode_string(buf)?;
  if buf.remaining() < 2 + 8 {
    return Err(WireError::Truncated);
  }
  let origin_port = buf.get_u16();
  let origin_uid = buf.get_u64();
  Ok((origin_system, origin_host, origin_port, origin_uid))
}

impl Codec<HandshakePdu> for HandshakeCodec {
  fn encode(&self, value: &HandshakePdu, buf: &mut BytesMut) -> Result<(), WireError> {
    match value {
      | HandshakePdu::Req(req) => {
        let len_pos = begin_frame(buf, KIND_HANDSHAKE_REQ);
        encode_body(req.origin_system(), req.origin_host(), req.origin_port(), req.origin_uid(), buf)?;
        patch_frame_length(buf, len_pos)
      },
      | HandshakePdu::Rsp(rsp) => {
        let len_pos = begin_frame(buf, KIND_HANDSHAKE_RSP);
        encode_body(rsp.origin_system(), rsp.origin_host(), rsp.origin_port(), rsp.origin_uid(), buf)?;
        patch_frame_length(buf, len_pos)
      },
    }
  }

  fn decode(&self, buf: &mut Bytes) -> Result<HandshakePdu, WireError> {
    let kind = peek_frame_kind(buf)?;
    match kind {
      | KIND_HANDSHAKE_REQ => {
        let _ = read_frame_header(buf, KIND_HANDSHAKE_REQ)?;
        let (system, host, port, uid) = decode_body(buf)?;
        Ok(HandshakePdu::Req(HandshakeReq::new(system, host, port, uid)))
      },
      | KIND_HANDSHAKE_RSP => {
        let _ = read_frame_header(buf, KIND_HANDSHAKE_RSP)?;
        let (system, host, port, uid) = decode_body(buf)?;
        Ok(HandshakePdu::Rsp(HandshakeRsp::new(system, host, port, uid)))
      },
      | _ => Err(WireError::UnknownKind),
    }
  }
}
