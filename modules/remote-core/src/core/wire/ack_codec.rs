//! Codec for [`AckPdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::core::wire::{
  ack_pdu::AckPdu,
  codec::Codec,
  frame_header::KIND_ACK,
  primitives::{begin_frame, patch_frame_length, read_frame_header},
  wire_error::WireError,
};

/// Zero-sized codec for [`AckPdu`].
#[derive(Clone, Copy, Debug, Default)]
pub struct AckCodec;

impl AckCodec {
  /// Creates a new [`AckCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Codec<AckPdu> for AckCodec {
  fn encode(&self, value: &AckPdu, buf: &mut BytesMut) -> Result<(), WireError> {
    let len_pos = begin_frame(buf, KIND_ACK);
    buf.put_u64(value.sequence_number());
    buf.put_u64(value.cumulative_ack());
    buf.put_u64(value.nack_bitmap());
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<AckPdu, WireError> {
    let _ = read_frame_header(buf, KIND_ACK)?;
    if buf.remaining() < 8 * 3 {
      return Err(WireError::Truncated);
    }
    let sequence_number = buf.get_u64();
    let cumulative_ack = buf.get_u64();
    let nack_bitmap = buf.get_u64();
    Ok(AckPdu::new(sequence_number, cumulative_ack, nack_bitmap))
  }
}
