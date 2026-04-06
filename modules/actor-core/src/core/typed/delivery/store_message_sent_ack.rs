//! Acknowledgment for a stored message sent event.

#[cfg(test)]
mod tests;

use crate::core::typed::delivery::SeqNr;

/// Acknowledgment returned by the durable queue after persisting a
/// [`MessageSent`](super::MessageSent) event.
///
/// Corresponds to Pekko's `DurableProducerQueue.StoreMessageSentAck`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMessageSentAck {
  stored_seq_nr: SeqNr,
}

impl StoreMessageSentAck {
  /// Creates a new acknowledgment with the given stored sequence number.
  #[must_use]
  pub const fn new(stored_seq_nr: SeqNr) -> Self {
    Self { stored_seq_nr }
  }

  /// Returns the sequence number that was stored.
  #[must_use]
  pub const fn stored_seq_nr(&self) -> SeqNr {
    self.stored_seq_nr
  }
}
