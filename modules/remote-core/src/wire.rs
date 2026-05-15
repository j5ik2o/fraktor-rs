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
mod compressed_text;
mod compression_advertisement;
mod compression_table;
mod compression_table_entry;
mod compression_table_kind;
mod control_codec;
mod control_pdu;
mod envelope_codec;
mod envelope_payload;
mod envelope_pdu;
mod flush_scope;
mod frame_header;
mod handshake_codec;
mod handshake_pdu;
mod handshake_req;
mod handshake_rsp;
mod primitives;
mod remote_deployment_codec;
mod remote_deployment_create_failure;
mod remote_deployment_create_request;
mod remote_deployment_create_success;
mod remote_deployment_failure_code;
mod remote_deployment_pdu;
mod wire_error;
mod wire_frame;

pub use ack_codec::AckCodec;
pub use ack_pdu::AckPdu;
pub use codec::Codec;
pub use compressed_text::CompressedText;
pub use compression_advertisement::CompressionAdvertisement;
pub use compression_table::CompressionTable;
pub use compression_table_entry::CompressionTableEntry;
pub use compression_table_kind::CompressionTableKind;
pub use control_codec::ControlCodec;
pub use control_pdu::ControlPdu;
pub use envelope_codec::EnvelopeCodec;
pub use envelope_payload::EnvelopePayload;
pub use envelope_pdu::EnvelopePdu;
pub use flush_scope::FlushScope;
pub use frame_header::{
  FRAME_KIND_OFFSET, FrameHeader, KIND_ACK, KIND_CONTROL, KIND_DEPLOYMENT, KIND_ENVELOPE, KIND_HANDSHAKE_REQ,
  KIND_HANDSHAKE_RSP, WIRE_VERSION, WIRE_VERSION_1, WIRE_VERSION_2, WIRE_VERSION_3, WIRE_VERSION_4,
};
pub use handshake_codec::HandshakeCodec;
pub use handshake_pdu::HandshakePdu;
pub use handshake_req::HandshakeReq;
pub use handshake_rsp::HandshakeRsp;
pub use remote_deployment_codec::RemoteDeploymentCodec;
pub use remote_deployment_create_failure::RemoteDeploymentCreateFailure;
pub use remote_deployment_create_request::RemoteDeploymentCreateRequest;
pub use remote_deployment_create_success::RemoteDeploymentCreateSuccess;
pub use remote_deployment_failure_code::RemoteDeploymentFailureCode;
pub use remote_deployment_pdu::RemoteDeploymentPdu;
pub use wire_error::WireError;
pub use wire_frame::WireFrame;
