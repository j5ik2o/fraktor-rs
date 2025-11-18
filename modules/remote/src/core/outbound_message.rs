//! Describes an outbound message queued for serialization.

use fraktor_actor_rs::core::{actor_prim::actor_path::ActorPath, messaging::AnyMessageGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{outbound_priority::OutboundPriority, remote_node_id::RemoteNodeId};

/// Message awaiting serialization and transport.
pub struct OutboundMessage<TB: RuntimeToolbox + 'static> {
  message:     AnyMessageGeneric<TB>,
  recipient:   ActorPath,
  remote_node: RemoteNodeId,
  reply_to:    Option<ActorPath>,
  priority:    OutboundPriority,
}

impl<TB: RuntimeToolbox + 'static> OutboundMessage<TB> {
  /// Creates a user-priority message.
  #[must_use]
  pub fn user(message: AnyMessageGeneric<TB>, recipient: ActorPath, remote_node: RemoteNodeId) -> Self {
    Self { message, recipient, remote_node, reply_to: None, priority: OutboundPriority::User }
  }

  /// Creates a system-priority message.
  #[must_use]
  pub fn system(message: AnyMessageGeneric<TB>, recipient: ActorPath, remote_node: RemoteNodeId) -> Self {
    Self { message, recipient, remote_node, reply_to: None, priority: OutboundPriority::System }
  }

  /// Attaches a reply-to actor path to the message.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: ActorPath) -> Self {
    self.reply_to = Some(reply_to);
    self
  }

  /// Returns the stored priority.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  pub(crate) fn into_parts(self) -> (AnyMessageGeneric<TB>, ActorPath, RemoteNodeId, Option<ActorPath>) {
    (self.message, self.recipient, self.remote_node, self.reply_to)
  }
}
