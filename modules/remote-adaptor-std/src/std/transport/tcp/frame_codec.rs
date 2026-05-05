//! `tokio_util::codec::{Encoder, Decoder}` wrapper around the core [`Codec<T>`]
//! implementations.

use bytes::BytesMut;
use fraktor_remote_core_rs::core::wire::{
  AckCodec, Codec, ControlCodec, EnvelopeCodec, FRAME_KIND_OFFSET, HandshakeCodec, KIND_ACK, KIND_CONTROL,
  KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP, WireError, WireFrame,
};
use tokio_util::codec::{Decoder, Encoder};

use super::frame_codec_error::FrameCodecError;

/// Minimum bytes required to inspect the frame header (`length(4)` + `version(1)` + `kind(1)`).
const FRAME_HEADER_LEN: usize = 6;
/// Minimum valid value for the declared frame length (`version + kind`).
const MIN_FRAME_LENGTH: usize = 2;
/// Default maximum allowed frame length declared in the 32-bit header.
///
/// This value includes bytes after the length field itself (`version + kind + body`).
const DEFAULT_MAXIMUM_FRAME_SIZE: usize = 256 * 1024;

/// Minimum accepted maximum frame size.
const MINIMUM_MAXIMUM_FRAME_SIZE: usize = 32 * 1024;

fn declared_frame_length(frame: &[u8]) -> Result<usize, FrameCodecError> {
  if frame.len() < FRAME_HEADER_LEN {
    return Err(FrameCodecError::from(WireError::InvalidFormat));
  }
  Ok(u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize)
}

/// Codec implementing `tokio_util::codec::{Encoder, Decoder}` for
/// [`crate::std::transport::tcp::WireFrame`].
///
/// Encode dispatches on the [`crate::std::transport::tcp::WireFrame`] variant and delegates to the
/// core `Codec<T>` implementor for that PDU. Decode peeks at the frame header to
/// determine the `kind` byte, splits off the complete frame bytes, and feeds
/// them through the corresponding core decoder.
#[derive(Clone, Copy, Debug)]
pub struct WireFrameCodec {
  maximum_frame_size: usize,
}

impl WireFrameCodec {
  /// Creates a new [`WireFrameCodec`].
  #[must_use]
  pub const fn new() -> Self {
    Self { maximum_frame_size: DEFAULT_MAXIMUM_FRAME_SIZE }
  }

  /// Creates a new [`WireFrameCodec`] with the given maximum frame size.
  ///
  /// # Panics
  ///
  /// Panics when `maximum_frame_size` is smaller than 32 KiB.
  #[must_use]
  pub const fn with_maximum_frame_size(maximum_frame_size: usize) -> Self {
    assert!(maximum_frame_size >= MINIMUM_MAXIMUM_FRAME_SIZE, "maximum frame size must be at least 32 KiB");
    Self { maximum_frame_size }
  }
}

impl Default for WireFrameCodec {
  fn default() -> Self {
    Self::new()
  }
}

impl Encoder<WireFrame> for WireFrameCodec {
  type Error = FrameCodecError;

  fn encode(&mut self, item: WireFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let mut frame = BytesMut::new();
    match item {
      | WireFrame::Envelope(pdu) => EnvelopeCodec::new().encode(&pdu, &mut frame),
      | WireFrame::Handshake(pdu) => HandshakeCodec::new().encode(&pdu, &mut frame),
      | WireFrame::Control(pdu) => ControlCodec::new().encode(&pdu, &mut frame),
      | WireFrame::Ack(pdu) => AckCodec::new().encode(&pdu, &mut frame),
    }
    .map_err(FrameCodecError::from)?;

    let length = declared_frame_length(&frame)?;
    if length > self.maximum_frame_size {
      return Err(FrameCodecError::from(WireError::FrameTooLarge));
    }
    dst.unsplit(frame);
    Ok(())
  }
}

impl Decoder for WireFrameCodec {
  type Error = FrameCodecError;
  type Item = WireFrame;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if src.len() < FRAME_HEADER_LEN {
      return Ok(None);
    }
    // バッファを消費せず length prefix だけ確認する。
    let length = declared_frame_length(src)?;
    if length < MIN_FRAME_LENGTH {
      return Err(FrameCodecError::from(WireError::InvalidFormat));
    }
    if length > self.maximum_frame_size {
      return Err(FrameCodecError::from(WireError::FrameTooLarge));
    }
    let total = 4 + length;
    if src.len() < total {
      // frame の残りが届くまで待つ。
      return Ok(None);
    }
    let kind = src[FRAME_KIND_OFFSET];
    // 完全な frame 1件だけを切り出して core decoder に渡す。
    let mut frame = src.split_to(total).freeze();
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
