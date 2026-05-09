//! Wire-protocol message between producer and consumer controllers.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::ProducerControllerCommand;
use crate::{TypedActorRef, delivery::SeqNr};

/// A message with a sequence number, sent from `ProducerController` to
/// `ConsumerController`.
///
/// This is the wire-protocol envelope. Application code rarely needs to
/// construct this directly; the `ProducerController` wraps outgoing messages
/// automatically.
#[derive(Clone)]
pub struct SequencedMessage<A>
where
  A: Clone + Send + Sync + 'static, {
  producer_id:         String,
  seq_nr:              SeqNr,
  message:             A,
  first:               bool,
  ack:                 bool,
  producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
}

impl<A> SequencedMessage<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a new sequenced message.
  pub(crate) const fn new(
    producer_id: String,
    seq_nr: SeqNr,
    message: A,
    first: bool,
    ack: bool,
    producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
  ) -> Self {
    Self { producer_id, seq_nr, message, first, ack, producer_controller }
  }

  /// Returns the producer identifier.
  #[must_use]
  pub fn producer_id(&self) -> &str {
    &self.producer_id
  }

  /// Returns the sequence number.
  #[must_use]
  pub const fn seq_nr(&self) -> SeqNr {
    self.seq_nr
  }

  /// Returns a reference to the wrapped message.
  #[must_use]
  pub const fn message(&self) -> &A {
    &self.message
  }

  /// Returns whether this is the first message in a new producer–consumer
  /// session.
  #[must_use]
  pub const fn first(&self) -> bool {
    self.first
  }

  /// Returns whether an explicit ack is requested.
  #[must_use]
  pub const fn ack(&self) -> bool {
    self.ack
  }

  /// Returns the producer controller reference (crate-internal).
  pub(crate) const fn producer_controller(&self) -> &TypedActorRef<ProducerControllerCommand<A>> {
    &self.producer_controller
  }

  /// Creates a copy with `first` set to `true`.
  pub(crate) fn as_first(&self) -> Self {
    Self { first: true, ..self.clone() }
  }
}
