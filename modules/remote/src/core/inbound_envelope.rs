//! Deserialized envelope waiting for delivery to the actor system.

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPath, event_stream::CorrelationId, messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{outbound_priority::OutboundPriority, remote_node_id::RemoteNodeId};

/// Represents a fully decoded inbound message alongside routing metadata.
pub struct InboundEnvelope<TB: RuntimeToolbox + 'static> {
  recipient:     ActorPath,
  remote_node:   RemoteNodeId,
  message:       AnyMessageGeneric<TB>,
  reply_to_path: Option<ActorPath>,
  correlation:   CorrelationId,
  priority:      OutboundPriority,
}

impl<TB: RuntimeToolbox + 'static> InboundEnvelope<TB> {
  /// Creates a new inbound envelope.
  #[must_use]
  pub fn new(
    recipient: ActorPath,
    remote_node: RemoteNodeId,
    message: AnyMessageGeneric<TB>,
    reply_to_path: Option<ActorPath>,
    correlation: CorrelationId,
    priority: OutboundPriority,
  ) -> Self {
    Self { recipient, remote_node, message, reply_to_path, correlation, priority }
  }

  /// Returns the intended recipient path.
  #[must_use]
  pub fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the originating remote node metadata.
  #[must_use]
  pub fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns a borrowed reference to the decoded message.
  #[must_use]
  pub fn message(&self) -> &AnyMessageGeneric<TB> {
    &self.message
  }

  /// Consumes the envelope and returns the owned message.
  #[must_use]
  pub fn into_message(self) -> AnyMessageGeneric<TB> {
    self.message
  }

  /// Returns the optional reply-to actor path provided by the remote sender.
  #[must_use]
  pub fn reply_to_path(&self) -> Option<&ActorPath> {
    self.reply_to_path.as_ref()
  }

  /// Returns the transport correlation identifier associated with the envelope.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation
  }

  /// Returns the priority originally assigned to the outbound message.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  /// Consumes the envelope and returns components required for delivery.
  pub fn into_delivery_parts(self) -> (ActorPath, AnyMessageGeneric<TB>, Option<ActorPath>) {
    (self.recipient, self.message, self.reply_to_path)
  }
}
