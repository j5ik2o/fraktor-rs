//! Codec for [`HandshakePdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::core::{
  address::{Address, UniqueAddress},
  wire::{
    codec::Codec,
    frame_header::{KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP},
    handshake_pdu::HandshakePdu,
    handshake_req::HandshakeReq,
    handshake_rsp::HandshakeRsp,
    primitives::{begin_frame, decode_string, encode_string, patch_frame_length, peek_frame_kind, read_frame_header},
    wire_error::WireError,
  },
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

fn encode_address(address: &Address, buf: &mut BytesMut) -> Result<(), WireError> {
  encode_string(address.system(), buf)?;
  encode_string(address.host(), buf)?;
  buf.put_u16(address.port());
  Ok(())
}

fn decode_address(buf: &mut Bytes) -> Result<Address, WireError> {
  let system = decode_string(buf)?;
  let host = decode_string(buf)?;
  if buf.remaining() < 2 {
    return Err(WireError::Truncated);
  }
  let port = buf.get_u16();
  Ok(Address::new(system, host, port))
}

fn encode_unique_address(address: &UniqueAddress, buf: &mut BytesMut) -> Result<(), WireError> {
  encode_address(address.address(), buf)?;
  buf.put_u64(address.uid());
  Ok(())
}

fn decode_unique_address(buf: &mut Bytes) -> Result<UniqueAddress, WireError> {
  let address = decode_address(buf)?;
  if buf.remaining() < 8 {
    return Err(WireError::Truncated);
  }
  let uid = buf.get_u64();
  Ok(UniqueAddress::new(address, uid))
}

impl Codec<HandshakePdu> for HandshakeCodec {
  fn encode(&self, value: &HandshakePdu, buf: &mut BytesMut) -> Result<(), WireError> {
    match value {
      | HandshakePdu::Req(req) => {
        let len_pos = begin_frame(buf, KIND_HANDSHAKE_REQ);
        encode_unique_address(req.from(), buf)?;
        encode_address(req.to(), buf)?;
        patch_frame_length(buf, len_pos)
      },
      | HandshakePdu::Rsp(rsp) => {
        let len_pos = begin_frame(buf, KIND_HANDSHAKE_RSP);
        encode_unique_address(rsp.from(), buf)?;
        patch_frame_length(buf, len_pos)
      },
    }
  }

  fn decode(&self, buf: &mut Bytes) -> Result<HandshakePdu, WireError> {
    let kind = peek_frame_kind(buf)?;
    match kind {
      | KIND_HANDSHAKE_REQ => {
        let (_header, _body_len) = read_frame_header(buf, KIND_HANDSHAKE_REQ)?;
        let from = decode_unique_address(buf)?;
        let to = decode_address(buf)?;
        Ok(HandshakePdu::Req(HandshakeReq::new(from, to)))
      },
      | KIND_HANDSHAKE_RSP => {
        let (_header, _body_len) = read_frame_header(buf, KIND_HANDSHAKE_RSP)?;
        let from = decode_unique_address(buf)?;
        Ok(HandshakePdu::Rsp(HandshakeRsp::new(from)))
      },
      | _ => Err(WireError::UnknownKind),
    }
  }
}
