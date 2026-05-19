//! Persisted fact representing a message that was sent.

#[cfg(test)]
#[path = "message_sent_test.rs"]
mod tests;

use crate::delivery::{ConfirmationQualifier, SeqNr};

/// A persisted fact representing a message that has been sent by the producer.
///
/// Stored by the durable queue so that unconfirmed messages can be recovered
/// after a crash.
///
/// Corresponds to Pekko's `DurableProducerQueue.MessageSent[A]`.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageSent<A>
where
  A: Clone + Send + Sync + 'static, {
  seq_nr:                 SeqNr,
  message:                A,
  ack:                    bool,
  confirmation_qualifier: ConfirmationQualifier,
  timestamp_millis:       u64,
}

impl<A> MessageSent<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a new `MessageSent` fact.
  #[must_use]
  pub const fn new(
    seq_nr: SeqNr,
    message: A,
    ack: bool,
    confirmation_qualifier: ConfirmationQualifier,
    timestamp_millis: u64,
  ) -> Self {
    Self { seq_nr, message, ack, confirmation_qualifier, timestamp_millis }
  }

  /// Returns the sequence number of this message.
  #[must_use]
  pub const fn seq_nr(&self) -> SeqNr {
    self.seq_nr
  }

  /// Returns a reference to the message payload.
  #[must_use]
  pub const fn message(&self) -> &A {
    &self.message
  }

  /// Returns whether the producer expects an acknowledgment for this message.
  #[must_use]
  pub const fn ack(&self) -> bool {
    self.ack
  }

  /// Returns the confirmation qualifier for this message.
  #[must_use]
  pub fn confirmation_qualifier(&self) -> &str {
    &self.confirmation_qualifier
  }

  /// Returns the timestamp in milliseconds when this message was sent.
  #[must_use]
  pub const fn timestamp_millis(&self) -> u64 {
    self.timestamp_millis
  }

  /// Returns a new `MessageSent` with the given confirmation qualifier.
  #[must_use]
  pub fn with_confirmation_qualifier<T>(self, qualifier: T) -> Self
  where
    T: Into<ConfirmationQualifier>, {
    Self { confirmation_qualifier: qualifier.into(), ..self }
  }

  /// Returns a new `MessageSent` with the given timestamp.
  #[must_use]
  pub fn with_timestamp_millis(self, timestamp_millis: u64) -> Self {
    Self { timestamp_millis, ..self }
  }
}
