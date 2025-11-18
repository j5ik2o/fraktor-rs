//! Serialized outbound frame metadata used by transports.

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPath,
  event_stream::CorrelationId,
  serialization::SerializedMessage,
};

use crate::core::{outbound_priority::OutboundPriority, remote_node_id::RemoteNodeId};

/// Fully serialized outbound message ready for transport framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingEnvelope {
  recipient:      ActorPath,
  remote_node:    RemoteNodeId,
  reply_to:       Option<ActorPath>,
  serialized:     SerializedMessage,
  correlation_id: CorrelationId,
  priority:       OutboundPriority,
}

impl RemotingEnvelope {
  /// Creates a new envelope with the provided components.
  #[must_use]
  pub const fn new(
    recipient: ActorPath,
    remote_node: RemoteNodeId,
    reply_to: Option<ActorPath>,
    serialized: SerializedMessage,
    correlation_id: CorrelationId,
    priority: OutboundPriority,
  ) -> Self {
    Self { recipient, remote_node, reply_to, serialized, correlation_id, priority }
  }

  /// Returns the fully qualified recipient path.
  #[must_use]
  pub fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the remote node metadata resolved during the handshake.
  #[must_use]
  pub fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns the optional reply target path.
  #[must_use]
  pub fn reply_to(&self) -> Option<&ActorPath> {
    self.reply_to.as_ref()
  }

  /// Returns the serialized payload.
  #[must_use]
  pub fn serialized_message(&self) -> &SerializedMessage {
    &self.serialized
  }

  /// Returns the correlation identifier shared with transport diagnostics.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Returns the logical priority of the message.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  /// Returns `true` when the envelope represents a system message.
  #[must_use]
  pub const fn is_system(&self) -> bool {
    self.priority.is_system()
  }
}
