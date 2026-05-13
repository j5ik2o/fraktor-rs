//! Public facade for durable producer queue protocol types.

#[cfg(test)]
#[path = "durable_producer_queue_test.rs"]
mod tests;

use crate::{
  TypedActorRef,
  delivery::{
    ConfirmationQualifier, DurableProducerQueueCommand, DurableProducerQueueState, MessageSent, SeqNr,
    StoreMessageSentAck,
  },
};

/// Pekko-compatible facade for the durable producer queue protocol family.
pub struct DurableProducerQueue;

impl DurableProducerQueue {
  /// Creates a `LoadState` command.
  #[must_use]
  pub const fn load_state<A>(reply_to: TypedActorRef<DurableProducerQueueState<A>>) -> DurableProducerQueueCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    DurableProducerQueueCommand::load_state(reply_to)
  }

  /// Creates a `StoreMessageSent` command.
  #[must_use]
  pub const fn store_message_sent<A>(
    sent: MessageSent<A>,
    reply_to: TypedActorRef<StoreMessageSentAck>,
  ) -> DurableProducerQueueCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    DurableProducerQueueCommand::store_message_sent(sent, reply_to)
  }

  /// Creates a `StoreMessageConfirmed` command.
  #[must_use]
  pub const fn store_message_confirmed<A>(
    seq_nr: SeqNr,
    confirmation_qualifier: ConfirmationQualifier,
    timestamp_millis: u64,
  ) -> DurableProducerQueueCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    DurableProducerQueueCommand::store_message_confirmed(seq_nr, confirmation_qualifier, timestamp_millis)
  }

  /// Returns the empty durable queue state.
  #[must_use]
  pub const fn empty_state<A>() -> DurableProducerQueueState<A>
  where
    A: Clone + Send + Sync + 'static, {
    DurableProducerQueueState::empty()
  }
}
