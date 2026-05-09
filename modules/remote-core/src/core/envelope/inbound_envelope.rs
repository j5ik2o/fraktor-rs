//! Inbound message envelope.

use fraktor_actor_core_rs::{
  actor::{actor_path::ActorPath, messaging::AnyMessage},
  event::stream::CorrelationId,
};

use crate::core::{address::RemoteNodeId, envelope::priority::OutboundPriority};

/// A fully decoded inbound message together with routing metadata.
///
/// Mirrors [`crate::core::envelope::OutboundEnvelope`] on the receiving side: it is
/// immutable data handed from the wire/receiver layer to the local delivery
/// pipeline.
#[derive(Debug)]
pub struct InboundEnvelope {
  recipient:      ActorPath,
  remote_node:    RemoteNodeId,
  message:        AnyMessage,
  sender:         Option<ActorPath>,
  correlation_id: CorrelationId,
  priority:       OutboundPriority,
}

impl InboundEnvelope {
  /// Creates a new [`InboundEnvelope`].
  #[must_use]
  pub const fn new(
    recipient: ActorPath,
    remote_node: RemoteNodeId,
    message: AnyMessage,
    sender: Option<ActorPath>,
    correlation_id: CorrelationId,
    priority: OutboundPriority,
  ) -> Self {
    Self { recipient, remote_node, message, sender, correlation_id, priority }
  }

  /// Returns the intended recipient path.
  #[must_use]
  pub const fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the originating remote node.
  #[must_use]
  pub const fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns the decoded message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }

  /// Returns the sender path, if provided by the remote end.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorPath> {
    self.sender.as_ref()
  }

  /// Returns the correlation identifier.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Returns the priority originally assigned by the sender.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  /// Consumes the envelope and returns its constituent parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorPath, RemoteNodeId, AnyMessage, Option<ActorPath>, CorrelationId, OutboundPriority) {
    (self.recipient, self.remote_node, self.message, self.sender, self.correlation_id, self.priority)
  }
}
