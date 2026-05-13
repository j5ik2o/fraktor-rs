//! Wire format for the remote subsystem: a compact big-endian binary format with
//! length-prefixed framing.
//!
//! See `openspec/changes/remote-redesign/specs/remote-core-wire-format/spec.md` for the
//! full contract. The crate intentionally avoids `prost` / `protobuf` (Decision 8)
//! and the [`Codec`] trait keeps the door open for an L2 Pekko Artery TCP wire
//! compatible codec implementation.

#[cfg(test)]
#[path = "wire_test.rs"]
mod tests;

mod ack_codec;
mod ack_pdu;
mod codec;
mod control_codec;
mod control_pdu;
mod envelope_codec;
mod envelope_payload;
mod envelope_pdu;
mod frame_header;
mod handshake_codec;
mod handshake_pdu;
mod handshake_req;
mod handshake_rsp;
mod primitives;
mod wire_error;
mod wire_frame;

pub use ack_codec::AckCodec;
pub use ack_pdu::AckPdu;
pub use codec::Codec;
pub use control_codec::ControlCodec;
pub use control_pdu::ControlPdu;
pub use envelope_codec::EnvelopeCodec;
pub use envelope_payload::EnvelopePayload;
pub use envelope_pdu::EnvelopePdu;
pub use frame_header::{
  FRAME_KIND_OFFSET, FrameHeader, KIND_ACK, KIND_CONTROL, KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP,
  WIRE_VERSION_1,
};
pub use handshake_codec::HandshakeCodec;
pub use handshake_pdu::HandshakePdu;
pub use handshake_req::HandshakeReq;
pub use handshake_rsp::HandshakeRsp;
pub use wire_error::WireError;
pub use wire_frame::WireFrame;
