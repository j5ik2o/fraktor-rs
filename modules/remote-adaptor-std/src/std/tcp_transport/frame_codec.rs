//! `tokio_util::codec::{Encoder, Decoder}` wrapper around the core [`Codec<T>`]
//! implementations.

use bytes::{Bytes, BytesMut};
use fraktor_remote_core_rs::core::wire::{
  AckCodec, Codec, ControlCodec, EnvelopeCodec, HandshakeCodec, KIND_ACK, KIND_CONTROL, KIND_ENVELOPE,
  KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP, WireError,
};
use tokio_util::codec::{Decoder, Encoder};

use crate::std::tcp_transport::{frame_codec_error::FrameCodecError, wire_frame::WireFrame};

/// Minimum bytes required to inspect the frame header (`length(4)` + `version(1)` + `kind(1)`).
const FRAME_HEADER_LEN: usize = 6;
/// Minimum valid value for the declared frame length (`version + kind`).
const MIN_FRAME_LENGTH: usize = 2;
/// Maximum allowed frame length declared in the 32-bit header.
///
/// This value includes bytes after the length field itself (`version + kind + body`).
const MAX_FRAME_LENGTH: usize = 16 * 1024 * 1024;

/// Zero-sized codec implementing `tokio_util::codec::{Encoder, Decoder}` for
/// [`crate::std::tcp_transport::WireFrame`].
///
/// Encode dispatches on the [`crate::std::tcp_transport::WireFrame`] variant and delegates to the
/// core `Codec<T>` implementor for that PDU. Decode peeks at the frame header to
/// determine the `kind` byte, splits off the complete frame bytes, and feeds
/// them back through the corresponding core decoder.
#[derive(Clone, Copy, Debug, Default)]
pub struct WireFrameCodec;

impl WireFrameCodec {
  /// Creates a new [`WireFrameCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Encoder<WireFrame> for WireFrameCodec {
  type Error = FrameCodecError;

  fn encode(&mut self, item: WireFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let result: Result<(), WireError> = match item {
      | WireFrame::Envelope(pdu) => EnvelopeCodec::new().encode(&pdu, dst),
      | WireFrame::Handshake(pdu) => HandshakeCodec::new().encode(&pdu, dst),
      | WireFrame::Control(pdu) => ControlCodec::new().encode(&pdu, dst),
      | WireFrame::Ack(pdu) => AckCodec::new().encode(&pdu, dst),
    };
    result.map_err(FrameCodecError::from)
  }
}

impl Decoder for WireFrameCodec {
  type Error = FrameCodecError;
  type Item = WireFrame;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if src.len() < FRAME_HEADER_LEN {
      return Ok(None);
    }
    // Peek at the length prefix without consuming the buffer.
    let length = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;
    if length < MIN_FRAME_LENGTH {
      return Err(FrameCodecError::from(WireError::InvalidFormat));
    }
    if length > MAX_FRAME_LENGTH {
      return Err(FrameCodecError::from(WireError::FrameTooLarge));
    }
    let total = 4 + length;
    if src.len() < total {
      // Wait for the remainder of the frame.
      return Ok(None);
    }
    // `kind` byte lives at `length(4) + version(1) = 5`.
    let kind = src[5];
    // Split off exactly one complete frame and feed it to the core decoder.
    let frame_bytes: Bytes = src.split_to(total).freeze();
    let mut frame = frame_bytes;
    let decoded: Result<WireFrame, WireError> = match kind {
      | KIND_ENVELOPE => EnvelopeCodec::new().decode(&mut frame).map(WireFrame::Envelope),
      | KIND_HANDSHAKE_REQ | KIND_HANDSHAKE_RSP => HandshakeCodec::new().decode(&mut frame).map(WireFrame::Handshake),
      | KIND_CONTROL => ControlCodec::new().decode(&mut frame).map(WireFrame::Control),
      | KIND_ACK => AckCodec::new().decode(&mut frame).map(WireFrame::Ack),
      | _ => Err(WireError::UnknownKind),
    };
    Ok(Some(decoded?))
  }
}
