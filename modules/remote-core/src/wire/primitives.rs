//! Internal big-endian primitive encode / decode helpers shared by every Codec.

use alloc::string::String;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  WireError,
  frame_header::{FrameHeader, WIRE_VERSION_1},
};

/// Writes a frame header placeholder (zero length) and returns the buffer offset
/// at which the length field starts. The caller must patch the length after the
/// body has been encoded, using [`patch_frame_length`].
pub(crate) fn begin_frame(buf: &mut BytesMut, kind: u8) -> usize {
  let len_pos = buf.len();
  buf.put_u32(0); // placeholder
  buf.put_u8(WIRE_VERSION_1);
  buf.put_u8(kind);
  len_pos
}

/// Patches the length field for a frame started with [`begin_frame`].
pub(crate) fn patch_frame_length(buf: &mut BytesMut, len_pos: usize) -> Result<(), WireError> {
  let total = buf.len();
  // Bytes after the length field itself (version + kind + body).
  let after_len = total.checked_sub(len_pos + 4).ok_or(WireError::InvalidFormat)?;
  if after_len > u32::MAX as usize {
    return Err(WireError::FrameTooLarge);
  }
  let length = after_len as u32;
  let bytes = length.to_be_bytes();
  buf[len_pos..len_pos + 4].copy_from_slice(&bytes);
  Ok(())
}

/// Reads and validates a complete frame header from `buf`, returning the header
/// together with the remaining body length (in bytes).
pub(crate) fn read_frame_header(buf: &mut Bytes, expected_kind: u8) -> Result<(FrameHeader, usize), WireError> {
  const HEADER_SIZE: usize = 4 + 1 + 1;
  if buf.remaining() < HEADER_SIZE {
    return Err(WireError::Truncated);
  }
  let length = buf.get_u32();
  if (length as usize) < 2 {
    // Must contain at least version + kind.
    return Err(WireError::InvalidFormat);
  }
  let body_len = length as usize - 2;
  // After consuming length, the remaining buffer must contain at least the
  // declared length worth of bytes.
  if buf.remaining() < length as usize {
    return Err(WireError::Truncated);
  }
  let version = buf.get_u8();
  if version != WIRE_VERSION_1 {
    return Err(WireError::UnknownVersion);
  }
  let kind = buf.get_u8();
  if kind != expected_kind {
    return Err(WireError::UnknownKind);
  }
  Ok((FrameHeader::new(length, version, kind), body_len))
}

/// Reads the `kind` byte from a frame header without consuming ownership of the
/// buffer. Useful for multi-kind dispatch (e.g. the handshake codec peeks at the
/// kind to distinguish `Req` from `Rsp`).
pub(crate) fn peek_frame_kind(buf: &Bytes) -> Result<u8, WireError> {
  const HEADER_SIZE: usize = 4 + 1 + 1;
  if buf.remaining() < HEADER_SIZE {
    return Err(WireError::Truncated);
  }
  // length(4) + version(1) = 5 → kind is at index 5.
  Ok(buf[5])
}

pub(crate) fn encode_string(value: &str, buf: &mut BytesMut) -> Result<(), WireError> {
  let bytes = value.as_bytes();
  if bytes.len() > u32::MAX as usize {
    return Err(WireError::InvalidFormat);
  }
  buf.put_u32(bytes.len() as u32);
  buf.put_slice(bytes);
  Ok(())
}

pub(crate) fn decode_string(buf: &mut Bytes) -> Result<String, WireError> {
  if buf.remaining() < 4 {
    return Err(WireError::Truncated);
  }
  let len = buf.get_u32() as usize;
  if buf.remaining() < len {
    return Err(WireError::Truncated);
  }
  let slice = buf.split_to(len);
  String::from_utf8(slice.to_vec()).map_err(|_| WireError::InvalidUtf8)
}

pub(crate) fn encode_option_string(value: Option<&str>, buf: &mut BytesMut) -> Result<(), WireError> {
  match value {
    | None => {
      buf.put_u8(0);
      Ok(())
    },
    | Some(s) => {
      buf.put_u8(1);
      encode_string(s, buf)
    },
  }
}

pub(crate) fn decode_option_string(buf: &mut Bytes) -> Result<Option<String>, WireError> {
  if buf.remaining() < 1 {
    return Err(WireError::Truncated);
  }
  let tag = buf.get_u8();
  match tag {
    | 0 => Ok(None),
    | 1 => Ok(Some(decode_string(buf)?)),
    | _ => Err(WireError::InvalidFormat),
  }
}

pub(crate) fn encode_bytes(value: &[u8], buf: &mut BytesMut) -> Result<(), WireError> {
  if value.len() > u32::MAX as usize {
    return Err(WireError::InvalidFormat);
  }
  buf.put_u32(value.len() as u32);
  buf.put_slice(value);
  Ok(())
}

pub(crate) fn decode_bytes(buf: &mut Bytes) -> Result<Bytes, WireError> {
  if buf.remaining() < 4 {
    return Err(WireError::Truncated);
  }
  let len = buf.get_u32() as usize;
  if buf.remaining() < len {
    return Err(WireError::Truncated);
  }
  Ok(buf.split_to(len))
}
