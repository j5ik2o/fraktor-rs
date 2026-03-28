//! Classic identity reply for actor discovery.

#[cfg(test)]
mod tests;

use crate::core::kernel::{actor::actor_ref::ActorRef, messaging::AnyMessage};

/// Reply sent for an [`Identify`](crate::core::kernel::messaging::Identify) request.
#[derive(Clone, Debug)]
pub struct ActorIdentity {
  correlation_id: AnyMessage,
  actor_ref:      Option<ActorRef>,
}

impl ActorIdentity {
  /// Creates a new identity reply.
  #[must_use]
  pub const fn new(correlation_id: AnyMessage, actor_ref: Option<ActorRef>) -> Self {
    Self { correlation_id, actor_ref }
  }

  /// Creates a reply for a discovered actor.
  #[must_use]
  pub const fn found(correlation_id: AnyMessage, actor_ref: ActorRef) -> Self {
    Self::new(correlation_id, Some(actor_ref))
  }

  /// Returns the correlation identifier from the original request.
  #[must_use]
  pub const fn correlation_id(&self) -> &AnyMessage {
    &self.correlation_id
  }

  /// Returns the discovered actor reference, if any.
  #[must_use]
  pub const fn actor_ref(&self) -> Option<&ActorRef> {
    self.actor_ref.as_ref()
  }
}
