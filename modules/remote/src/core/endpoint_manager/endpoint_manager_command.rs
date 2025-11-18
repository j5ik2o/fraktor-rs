//! Commands handled by the endpoint manager.

use alloc::{string::String, vec::Vec};

use super::remote_node_id::RemoteNodeId;

/// Commands accepted by the endpoint manager state machine.
#[derive(Clone, Debug, PartialEq)]
pub enum EndpointManagerCommand {
  /// Register an inbound handle to start a handshake.
  RegisterInbound {
    /// Authority identifier of the inbound connection.
    authority: String,
  },
  /// Begin association to a transport endpoint.
  Associate {
    /// Authority identifier that should be dialled.
    authority: String,
  },
  /// Confirm handshake with the remote node id.
  HandshakeCompleted {
    /// Authority being confirmed.
    authority: String,
    /// Remote node descriptor that completed the handshake.
    remote:    RemoteNodeId,
  },
  /// Flush deferred messages for the authority.
  FlushDeferred {
    /// Authority whose queue should be flushed.
    authority: String,
  },
  /// Attach a payload to be deferred.
  DeferMessage {
    /// Authority whose queue will store the message.
    authority: String,
    /// Deferred payload bytes.
    payload:   Vec<u8>,
  },
}
