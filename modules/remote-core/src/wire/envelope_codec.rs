//! Codec for [`EnvelopePdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::wire::{
  codec::Codec,
  envelope_pdu::EnvelopePdu,
  frame_header::KIND_ENVELOPE,
  primitives::{
    begin_frame, decode_bytes, decode_option_string, decode_string, encode_bytes, encode_option_string, encode_string,
    patch_frame_length, read_frame_header,
  },
  wire_error::WireError,
};

/// Zero-sized codec for [`EnvelopePdu`] producing the `kind = 0x01` frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvelopeCodec;

impl EnvelopeCodec {
  /// Creates a new [`EnvelopeCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Codec<EnvelopePdu> for EnvelopeCodec {
  fn encode(&self, value: &EnvelopePdu, buf: &mut BytesMut) -> Result<(), WireError> {
    let len_pos = begin_frame(buf, KIND_ENVELOPE);
    encode_string(value.recipient_path(), buf)?;
    encode_option_string(value.sender_path(), buf)?;
    buf.put_u64(value.correlation_hi());
    buf.put_u32(value.correlation_lo());
    buf.put_u8(value.priority());
    encode_bytes(value.payload(), buf)?;
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<EnvelopePdu, WireError> {
    let _ = read_frame_header(buf, KIND_ENVELOPE)?;
    let recipient_path = decode_string(buf)?;
    let sender_path = decode_option_string(buf)?;
    if buf.remaining() < 8 + 4 + 1 {
      return Err(WireError::Truncated);
    }
    let correlation_hi = buf.get_u64();
    let correlation_lo = buf.get_u32();
    let priority = buf.get_u8();
    let payload = decode_bytes(buf)?;
    Ok(EnvelopePdu::new(recipient_path, sender_path, correlation_hi, correlation_lo, priority, payload))
  }
}
