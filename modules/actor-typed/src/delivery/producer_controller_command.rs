//! Commands accepted by the producer controller actor.

#[cfg(test)]
mod tests;

use crate::{
  TypedActorRef,
  delivery::{
    ConsumerControllerCommand, DurableProducerQueueState, ProducerControllerRequestNext, SeqNr, StoreMessageSentAck,
  },
};

/// Commands handled by
/// [`ProducerController`](crate::delivery::ProducerController).
///
/// User code constructs commands through
/// [`ProducerController`](crate::delivery::ProducerController)
/// factory methods. Internal protocol messages are crate-private.
#[derive(Clone)]
pub struct ProducerControllerCommand<A>(pub(crate) ProducerControllerCommandKind<A>)
where
  A: Clone + Send + Sync + 'static;

#[derive(Clone)]
pub(crate) enum ProducerControllerCommandKind<A>
where
  A: Clone + Send + Sync + 'static, {
  /// Initial message from the producer actor.
  Start { producer: TypedActorRef<ProducerControllerRequestNext<A>> },
  /// Register a consumer controller to this producer controller.
  RegisterConsumer { consumer_controller: TypedActorRef<ConsumerControllerCommand<A>> },
  /// A message from the producer (via `send_next_to`).
  Msg { message: A },
  /// Demand request from the consumer controller.
  Request { confirmed_seq_nr: SeqNr, request_up_to_seq_nr: SeqNr, support_resend: bool },
  /// Resend request from the consumer controller.
  Resend { from_seq_nr: SeqNr },
  /// Ack from the consumer controller.
  Ack { confirmed_seq_nr: SeqNr },
  /// Loaded durable queue state.
  DurableQueueLoaded { state: DurableProducerQueueState<A> },
  /// Acknowledgment that a sent message fact was stored.
  DurableQueueMessageStored { ack: StoreMessageSentAck },
  /// Internal timer: durable queue load timed out.
  DurableQueueLoadTimedOut { attempt: u32 },
  /// Internal timer: durable queue store timed out.
  DurableQueueStoreTimedOut { seq_nr: SeqNr, attempt: u32 },
  /// Internal timer: resend the first unconfirmed message.
  ResendFirstUnconfirmed { seq_nr: SeqNr },
}

impl<A> ProducerControllerCommand<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a `Start` command.
  pub(crate) const fn start(producer: TypedActorRef<ProducerControllerRequestNext<A>>) -> Self {
    Self(ProducerControllerCommandKind::Start { producer })
  }

  /// Creates a `RegisterConsumer` command.
  pub(crate) const fn register_consumer(consumer_controller: TypedActorRef<ConsumerControllerCommand<A>>) -> Self {
    Self(ProducerControllerCommandKind::RegisterConsumer { consumer_controller })
  }

  /// Creates a `Msg` command (internal, from producer via send_next_to adapter).
  pub(crate) const fn msg(message: A) -> Self {
    Self(ProducerControllerCommandKind::Msg { message })
  }

  /// Creates a `Request` command (internal, from consumer controller).
  pub(crate) const fn request(confirmed_seq_nr: SeqNr, request_up_to_seq_nr: SeqNr, support_resend: bool) -> Self {
    Self(ProducerControllerCommandKind::Request { confirmed_seq_nr, request_up_to_seq_nr, support_resend })
  }

  /// Creates a `Resend` command (internal, from consumer controller).
  pub(crate) const fn resend(from_seq_nr: SeqNr) -> Self {
    Self(ProducerControllerCommandKind::Resend { from_seq_nr })
  }

  /// Creates an `Ack` command (internal, from consumer controller).
  pub(crate) const fn ack(confirmed_seq_nr: SeqNr) -> Self {
    Self(ProducerControllerCommandKind::Ack { confirmed_seq_nr })
  }

  /// Creates a `DurableQueueLoaded` command (internal).
  pub(crate) const fn durable_queue_loaded(state: DurableProducerQueueState<A>) -> Self {
    Self(ProducerControllerCommandKind::DurableQueueLoaded { state })
  }

  /// Creates a `DurableQueueMessageStored` command (internal).
  pub(crate) const fn durable_queue_message_stored(ack: StoreMessageSentAck) -> Self {
    Self(ProducerControllerCommandKind::DurableQueueMessageStored { ack })
  }

  /// Creates a `DurableQueueLoadTimedOut` command (internal timer).
  pub(crate) const fn durable_queue_load_timed_out(attempt: u32) -> Self {
    Self(ProducerControllerCommandKind::DurableQueueLoadTimedOut { attempt })
  }

  /// Creates a `DurableQueueStoreTimedOut` command (internal timer).
  pub(crate) const fn durable_queue_store_timed_out(seq_nr: SeqNr, attempt: u32) -> Self {
    Self(ProducerControllerCommandKind::DurableQueueStoreTimedOut { seq_nr, attempt })
  }

  /// Creates a `ResendFirstUnconfirmed` command (internal timer).
  pub(crate) const fn resend_first_unconfirmed(seq_nr: SeqNr) -> Self {
    Self(ProducerControllerCommandKind::ResendFirstUnconfirmed { seq_nr })
  }

  /// Returns a reference to the command kind.
  pub(crate) const fn kind(&self) -> &ProducerControllerCommandKind<A> {
    &self.0
  }
}
