//! Demand signal from the work-pulling producer controller to the producer.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::core::typed::{TypedActorRef, delivery::SeqNr};

/// Sent from `WorkPullingProducerController` to the producer to signal that
/// it may send one message.
///
/// The producer sends the next message to `send_next_to`. The
/// `current_seq_nr` and `confirmed_seq_nr` are informational.
#[derive(Clone)]
pub struct WorkPullingProducerControllerRequestNext<A>
where
  A: Clone + Send + Sync + 'static, {
  producer_id:      String,
  current_seq_nr:   SeqNr,
  confirmed_seq_nr: SeqNr,
  send_next_to:     TypedActorRef<A>,
}

impl<A> WorkPullingProducerControllerRequestNext<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a new request-next signal.
  pub(crate) const fn new(
    producer_id: String,
    current_seq_nr: SeqNr,
    confirmed_seq_nr: SeqNr,
    send_next_to: TypedActorRef<A>,
  ) -> Self {
    Self { producer_id, current_seq_nr, confirmed_seq_nr, send_next_to }
  }

  /// Returns the producer identifier.
  #[must_use]
  pub fn producer_id(&self) -> &str {
    &self.producer_id
  }

  /// Returns the sequence number that will be assigned to the next message.
  #[must_use]
  pub const fn current_seq_nr(&self) -> SeqNr {
    self.current_seq_nr
  }

  /// Returns the highest sequence number confirmed by consumers.
  #[must_use]
  pub const fn confirmed_seq_nr(&self) -> SeqNr {
    self.confirmed_seq_nr
  }

  /// Returns the actor reference the producer should send the next message to.
  #[must_use]
  pub const fn send_next_to(&self) -> &TypedActorRef<A> {
    &self.send_next_to
  }
}
