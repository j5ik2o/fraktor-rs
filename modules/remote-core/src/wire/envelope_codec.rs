//! Codec for [`EnvelopePdu`].

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{
  envelope::OutboundPriority,
  wire::{
    codec::Codec,
    compressed_text::{
      decode_compressed_text, decode_option_compressed_text, encode_compressed_text, encode_option_compressed_text,
    },
    envelope_pdu::EnvelopePdu,
    frame_header::KIND_ENVELOPE,
    primitives::{begin_frame, decode_bytes, encode_bytes, patch_frame_length, read_frame_header},
    wire_error::WireError,
  },
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
    encode_compressed_text(value.recipient_path_metadata(), buf)?;
    encode_option_compressed_text(value.sender_path_metadata(), buf)?;
    buf.put_u64(value.correlation_hi());
    buf.put_u32(value.correlation_lo());
    buf.put_u8(value.priority());
    encode_redelivery_sequence(value.priority(), value.redelivery_sequence(), buf)?;
    buf.put_u32(value.serializer_id());
    encode_option_compressed_text(value.manifest_metadata(), buf)?;
    encode_bytes(value.payload(), buf)?;
    patch_frame_length(buf, len_pos)
  }

  fn decode(&self, buf: &mut Bytes) -> Result<EnvelopePdu, WireError> {
    read_frame_header(buf, KIND_ENVELOPE)?;
    let recipient_path = decode_compressed_text(buf)?;
    let sender_path = decode_option_compressed_text(buf)?;
    if buf.remaining() < 8 + 4 + 1 + 1 + 4 {
      return Err(WireError::Truncated);
    }
    let correlation_hi = buf.get_u64();
    let correlation_lo = buf.get_u32();
    let priority = buf.get_u8();
    let redelivery_sequence = decode_redelivery_sequence(priority, buf)?;
    if buf.remaining() < 4 {
      return Err(WireError::Truncated);
    }
    let serializer_id = buf.get_u32();
    let manifest = decode_option_compressed_text(buf)?;
    let payload = decode_bytes(buf)?;
    Ok(EnvelopePdu::new_with_metadata(
      recipient_path,
      sender_path,
      (correlation_hi, correlation_lo),
      priority,
      serializer_id,
      manifest,
      payload,
    ))
    .map(|pdu| pdu.with_redelivery_sequence(redelivery_sequence))
  }
}

fn encode_redelivery_sequence(priority: u8, sequence: Option<u64>, buf: &mut BytesMut) -> Result<(), WireError> {
  match (OutboundPriority::from_wire(priority), sequence) {
    | (Some(OutboundPriority::System), Some(sequence)) => {
      buf.put_u8(1);
      buf.put_u64(sequence);
      Ok(())
    },
    | (Some(OutboundPriority::User), None) => {
      buf.put_u8(0);
      Ok(())
    },
    | _ => Err(WireError::InvalidFormat),
  }
}

fn decode_redelivery_sequence(priority: u8, buf: &mut Bytes) -> Result<Option<u64>, WireError> {
  let Some(priority) = OutboundPriority::from_wire(priority) else {
    return Err(WireError::InvalidFormat);
  };
  let flag = buf.get_u8();
  let sequence = match flag {
    | 0 => None,
    | 1 => {
      if buf.remaining() < 8 {
        return Err(WireError::Truncated);
      }
      Some(buf.get_u64())
    },
    | _ => return Err(WireError::InvalidFormat),
  };
  match (priority, sequence) {
    | (OutboundPriority::System, Some(sequence)) => Ok(Some(sequence)),
    | (OutboundPriority::User, None) => Ok(None),
    | _ => Err(WireError::InvalidFormat),
  }
}
