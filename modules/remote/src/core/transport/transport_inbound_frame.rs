//! Represents an inbound payload delivered by a transport implementation.

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::event_stream::CorrelationId;

/// Metadata describing a frame received from a remote peer.
pub struct InboundFrame {
  local_authority: String,
  remote_address:  String,
  payload:         Vec<u8>,
  correlation_id:  CorrelationId,
}

impl InboundFrame {
  /// Creates a new inbound frame descriptor.
  #[must_use]
  pub fn new(
    local_authority: impl Into<String>,
    remote_address: impl Into<String>,
    payload: Vec<u8>,
    correlation_id: CorrelationId,
  ) -> Self {
    Self { local_authority: local_authority.into(), remote_address: remote_address.into(), payload, correlation_id }
  }

  /// Returns the local authority (listener) that accepted the frame.
  #[must_use]
  pub fn local_authority(&self) -> &str {
    &self.local_authority
  }

  /// Returns the remote socket address of the peer.
  #[must_use]
  pub fn remote_address(&self) -> &str {
    &self.remote_address
  }

  /// Returns the raw payload bytes after length-prefix decoding.
  #[must_use]
  pub fn payload(&self) -> &[u8] {
    &self.payload
  }

  /// Returns the correlation identifier associated with the frame.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }
}
