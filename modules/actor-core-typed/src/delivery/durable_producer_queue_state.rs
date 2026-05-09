//! Durable producer queue state for crash recovery.

#[cfg(test)]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use crate::delivery::{ConfirmationQualifier, MessageSent, SeqNr};

/// Snapshot of the durable producer queue's persisted state.
///
/// All mutation methods consume `self` and return a new instance,
/// following Pekko's immutable `State.copy(...)` pattern.
///
/// Corresponds to Pekko's `DurableProducerQueue.State[A]`.
#[derive(Debug, Clone)]
pub struct DurableProducerQueueState<A>
where
  A: Clone + Send + Sync + 'static, {
  current_seq_nr:           SeqNr,
  highest_confirmed_seq_nr: SeqNr,
  confirmed_seq_nr:         BTreeMap<ConfirmationQualifier, (SeqNr, u64)>,
  unconfirmed:              Vec<MessageSent<A>>,
}

impl<A> DurableProducerQueueState<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates an empty initial state.
  ///
  /// Corresponds to Pekko's `State.empty`.
  #[must_use]
  pub const fn empty() -> Self {
    Self {
      current_seq_nr:           1,
      highest_confirmed_seq_nr: 0,
      confirmed_seq_nr:         BTreeMap::new(),
      unconfirmed:              Vec::new(),
    }
  }

  /// Returns the next sequence number to be assigned.
  #[must_use]
  pub const fn current_seq_nr(&self) -> SeqNr {
    self.current_seq_nr
  }

  /// Returns the highest confirmed sequence number across all qualifiers.
  #[must_use]
  pub const fn highest_confirmed_seq_nr(&self) -> SeqNr {
    self.highest_confirmed_seq_nr
  }

  /// Returns the per-qualifier confirmed sequence number map.
  #[must_use]
  pub const fn confirmed_seq_nr(&self) -> &BTreeMap<ConfirmationQualifier, (SeqNr, u64)> {
    &self.confirmed_seq_nr
  }

  /// Returns the list of unconfirmed messages.
  #[must_use]
  pub fn unconfirmed(&self) -> &[MessageSent<A>] {
    &self.unconfirmed
  }

  /// Adds a sent message and advances the current sequence number.
  ///
  /// Returns a new state with the message appended to `unconfirmed` and
  /// `current_seq_nr` set to `sent.seq_nr() + 1`.
  #[must_use]
  pub fn add_message_sent(mut self, sent: MessageSent<A>) -> Self {
    let next_seq_nr = sent.seq_nr() + 1;
    self.unconfirmed.push(sent);
    self.current_seq_nr = next_seq_nr;
    self
  }

  /// Records a confirmation up to the given sequence number for a qualifier.
  ///
  /// Removes unconfirmed messages that match the qualifier and have a
  /// sequence number less than or equal to `seq_nr`. Updates
  /// `highest_confirmed_seq_nr` to the maximum of the current value and
  /// `seq_nr`.
  #[must_use]
  pub fn confirmed(mut self, seq_nr: SeqNr, qualifier: ConfirmationQualifier, timestamp_millis: u64) -> Self {
    // qualifier が空文字（NO_QUALIFIER）の場合は qualifier を無視してフィルタリングする
    if qualifier.is_empty() {
      self.unconfirmed.retain(|msg| msg.seq_nr() > seq_nr);
    } else {
      self.unconfirmed.retain(|msg| !(msg.confirmation_qualifier() == qualifier && msg.seq_nr() <= seq_nr));
    }

    if seq_nr > self.highest_confirmed_seq_nr {
      self.highest_confirmed_seq_nr = seq_nr;
    }

    self.confirmed_seq_nr.insert(qualifier, (seq_nr, timestamp_millis));
    self
  }

  /// Removes entries from the confirmed sequence number map for the given
  /// qualifiers.
  ///
  /// Used to clean up tracking state for qualifiers that are no longer
  /// active.
  #[must_use]
  pub fn cleanup(mut self, qualifiers: &BTreeSet<String>) -> Self {
    for q in qualifiers {
      self.confirmed_seq_nr.remove(q);
    }
    self
  }
}
