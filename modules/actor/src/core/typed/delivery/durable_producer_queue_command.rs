//! Commands for the durable producer queue actor.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::core::typed::{
  TypedActorRef,
  delivery::{ConfirmationQualifier, DurableProducerQueueState, MessageSent, SeqNr, StoreMessageSentAck},
};

/// Commands accepted by a durable producer queue actor.
///
/// The durable queue actor persists sent messages and confirmations so that
/// unconfirmed messages can be recovered after a crash.
///
/// Corresponds to Pekko's `DurableProducerQueue.Command[A]`.
#[derive(Debug)]
pub enum DurableProducerQueueCommand<A>
where
  A: Clone + Send + Sync + 'static, {
  /// Load the persisted state on startup.
  ///
  /// Corresponds to Pekko's `DurableProducerQueue.LoadState`.
  LoadState {
    /// The actor to reply to with the loaded state.
    reply_to: TypedActorRef<DurableProducerQueueState<A>>,
  },
  /// Persist a sent message event.
  ///
  /// Corresponds to Pekko's `DurableProducerQueue.StoreMessageSent`.
  StoreMessageSent {
    /// The message sent event to persist.
    sent:     MessageSent<A>,
    /// The actor to reply to with the acknowledgment.
    reply_to: TypedActorRef<StoreMessageSentAck>,
  },
  /// Persist a confirmation event.
  ///
  /// Corresponds to Pekko's `DurableProducerQueue.StoreMessageConfirmed`.
  StoreMessageConfirmed {
    /// The confirmed sequence number.
    seq_nr:                 SeqNr,
    /// The confirmation qualifier.
    confirmation_qualifier: ConfirmationQualifier,
    /// The timestamp in milliseconds.
    timestamp_millis:       u64,
  },
}

impl<A> DurableProducerQueueCommand<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a `LoadState` command.
  #[must_use]
  pub const fn load_state(reply_to: TypedActorRef<DurableProducerQueueState<A>>) -> Self {
    Self::LoadState { reply_to }
  }

  /// Creates a `StoreMessageSent` command.
  #[must_use]
  pub const fn store_message_sent(sent: MessageSent<A>, reply_to: TypedActorRef<StoreMessageSentAck>) -> Self {
    Self::StoreMessageSent { sent, reply_to }
  }

  /// Creates a `StoreMessageConfirmed` command.
  #[must_use]
  pub const fn store_message_confirmed(seq_nr: SeqNr, confirmation_qualifier: String, timestamp_millis: u64) -> Self {
    Self::StoreMessageConfirmed { seq_nr, confirmation_qualifier, timestamp_millis }
  }
}
