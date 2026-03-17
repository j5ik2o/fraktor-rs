//! Delivery wrapper sent from the consumer controller to the consumer.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::ConsumerControllerConfirmed;
use crate::core::typed::{actor::TypedActorRef, delivery::SeqNr};

/// A message wrapped with delivery metadata, sent from `ConsumerController`
/// to the destination consumer actor.
///
/// The consumer is expected to process the message and reply with
/// [`ConsumerControllerConfirmed`] to the `confirm_to` reference.
#[derive(Clone)]
pub struct ConsumerControllerDelivery<A>
where
  A: Clone + Send + Sync + 'static, {
  message:     A,
  confirm_to:  TypedActorRef<ConsumerControllerConfirmed>,
  producer_id: String,
  seq_nr:      SeqNr,
}

impl<A> ConsumerControllerDelivery<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a new delivery envelope.
  pub(crate) const fn new(
    message: A,
    confirm_to: TypedActorRef<ConsumerControllerConfirmed>,
    producer_id: String,
    seq_nr: SeqNr,
  ) -> Self {
    Self { message, confirm_to, producer_id, seq_nr }
  }

  /// Returns a reference to the wrapped message.
  #[must_use]
  pub const fn message(&self) -> &A {
    &self.message
  }

  /// Returns the actor reference to send confirmation to.
  #[must_use]
  pub const fn confirm_to(&self) -> &TypedActorRef<ConsumerControllerConfirmed> {
    &self.confirm_to
  }

  /// Returns the producer identifier.
  #[must_use]
  pub fn producer_id(&self) -> &str {
    &self.producer_id
  }

  /// Returns the sequence number of this delivery.
  #[must_use]
  pub const fn seq_nr(&self) -> SeqNr {
    self.seq_nr
  }
}
