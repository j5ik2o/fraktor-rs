//! Outbound message envelope.

use fraktor_actor_core_rs::{
  actor::{actor_path::ActorPath, messaging::AnyMessage},
  event::stream::CorrelationId,
};

use crate::core::{address::RemoteNodeId, envelope::priority::OutboundPriority};

/// An outbound message queued for serialization and transport to a remote node.
///
/// This is immutable data: all fields are private and exposed through accessors,
/// and the envelope cannot be mutated after construction. The higher-level
/// `Association` state machine moves outbound envelopes through its `SendQueue`
/// without ever re-writing them.
///
/// `Clone` is provided so the outbound runtime can buffer a copy before handing
/// the envelope off to a fallible `RemoteTransport::send`; on transient send
/// failure the buffered copy is re-enqueued through `Association::enqueue` so
/// no message is silently lost across reconnect.
#[derive(Debug, Clone)]
pub struct OutboundEnvelope {
  recipient:      ActorPath,
  sender:         Option<ActorPath>,
  message:        AnyMessage,
  priority:       OutboundPriority,
  remote_node:    RemoteNodeId,
  correlation_id: CorrelationId,
}

impl OutboundEnvelope {
  /// Creates a new [`OutboundEnvelope`].
  #[must_use]
  pub const fn new(
    recipient: ActorPath,
    sender: Option<ActorPath>,
    message: AnyMessage,
    priority: OutboundPriority,
    remote_node: RemoteNodeId,
    correlation_id: CorrelationId,
  ) -> Self {
    Self { recipient, sender, message, priority, remote_node, correlation_id }
  }

  /// Returns the recipient actor path.
  #[must_use]
  pub const fn recipient(&self) -> &ActorPath {
    &self.recipient
  }

  /// Returns the sender actor path, if provided by the caller.
  #[must_use]
  pub const fn sender(&self) -> Option<&ActorPath> {
    self.sender.as_ref()
  }

  /// Returns the payload message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }

  /// Returns the priority assigned by the sender.
  #[must_use]
  pub const fn priority(&self) -> OutboundPriority {
    self.priority
  }

  /// Returns the target remote node identifier.
  #[must_use]
  pub const fn remote_node(&self) -> &RemoteNodeId {
    &self.remote_node
  }

  /// Returns the correlation identifier carried through the remote pipeline.
  #[must_use]
  pub const fn correlation_id(&self) -> CorrelationId {
    self.correlation_id
  }

  /// Consumes the envelope and returns its constituent parts.
  #[must_use]
  pub fn into_parts(self) -> (ActorPath, Option<ActorPath>, AnyMessage, OutboundPriority, RemoteNodeId, CorrelationId) {
    (self.recipient, self.sender, self.message, self.priority, self.remote_node, self.correlation_id)
  }
}
