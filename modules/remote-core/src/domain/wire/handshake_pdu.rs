//! Handshake PDU enum wrapping request / response variants.

use crate::domain::wire::{handshake_req::HandshakeReq, handshake_rsp::HandshakeRsp};

/// Wire-level handshake PDU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandshakePdu {
  /// Handshake request sent by the initiating node.
  Req(HandshakeReq),
  /// Handshake response sent by the accepting node.
  Rsp(HandshakeRsp),
}
