//! Describes an outbound message queued for serialization.

use fraktor_actor_rs::core::kernel::{actor::actor_path::ActorPath, messaging::AnyMessage};

use super::priority::OutboundPriority;
use crate::core::remote_node_id::RemoteNodeId;

/// Message awaiting serialization and transport.
pub struct OutboundMessage {
  message:     AnyMessage,
  recipient:   ActorPath,
  remote_node: RemoteNodeId,
  sender:      Option<ActorPath>,
  priority:    OutboundPriority,
}

impl OutboundMessage {
  /// Creates a user-priority message.
  #[must_use]
  pub fn user(message: AnyMessage, recipient: ActorPath, remote_node: RemoteNodeId) -> Self {
    Self { message, recipient, remote_node, sender: None, priority: OutboundPriority::User }
  }

  /// Creates a system-priority message.
  #[must_use]
  pub fn system(message: AnyMessage, recipient: ActorPath, remote_node: RemoteNodeId) -> Self {
    Self { message, recipient, remote_node, sender: None, priority: OutboundPriority::System }
  }

  /// Attaches a sender actor path to the message.
  #[must_use]
  pub fn with_sender(mut self, sender: ActorPath) -> Self {
    self.sender = Some(sender);
    self
  }

  /// Returns the stored priority.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  pub(crate) fn into_parts(self) -> (AnyMessage, ActorPath, RemoteNodeId, Option<ActorPath>) {
    (self.message, self.recipient, self.remote_node, self.sender)
  }
}
