#[cfg(test)]
mod tests;

use fraktor_actor_core_rs::actor::{actor_ref::ActorRef, messaging::AnyMessage};

/// Message envelope delivered to a stage actor receive callback.
pub struct StageActorEnvelope {
  sender:  ActorRef,
  message: AnyMessage,
}

impl StageActorEnvelope {
  /// Creates a new envelope from the explicit sender and message.
  #[must_use]
  pub const fn new(sender: ActorRef, message: AnyMessage) -> Self {
    Self { sender, message }
  }

  /// Returns the actor that sent the message.
  #[must_use]
  pub const fn sender(&self) -> &ActorRef {
    &self.sender
  }

  /// Returns the delivered message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }

  pub(in crate::core) fn into_message(self) -> AnyMessage {
    self.message
  }
}
