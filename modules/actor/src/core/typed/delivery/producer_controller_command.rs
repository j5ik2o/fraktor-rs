//! Commands accepted by the producer controller actor.

#[cfg(test)]
mod tests;

use crate::core::typed::{
  actor::TypedActorRef,
  delivery::{ConsumerControllerCommand, ProducerControllerRequestNext, SeqNr},
};

/// Commands handled by
/// [`ProducerController`](crate::core::typed::delivery::ProducerController).
///
/// User code constructs commands through
/// [`ProducerController`](crate::core::typed::delivery::ProducerController)
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
  /// A message from the producer with confirmation (via `ask_next_to`).
  #[allow(dead_code)]
  MsgWithConfirmation { message: A, reply_to: TypedActorRef<SeqNr> },
  /// Demand request from the consumer controller.
  Request {
    confirmed_seq_nr:     SeqNr,
    request_up_to_seq_nr: SeqNr,
    support_resend:       bool,
    #[allow(dead_code)]
    via_timeout:          bool,
  },
  /// Resend request from the consumer controller.
  Resend { from_seq_nr: SeqNr },
  /// Ack from the consumer controller.
  Ack { confirmed_seq_nr: SeqNr },
  /// Internal timer: resend the first unconfirmed message.
  #[allow(dead_code)]
  ResendFirstUnconfirmed,
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

  /// Creates a `MsgWithConfirmation` command.
  #[allow(dead_code)]
  pub(crate) const fn msg_with_confirmation(message: A, reply_to: TypedActorRef<SeqNr>) -> Self {
    Self(ProducerControllerCommandKind::MsgWithConfirmation { message, reply_to })
  }

  /// Creates a `Request` command (internal, from consumer controller).
  pub(crate) const fn request(
    confirmed_seq_nr: SeqNr,
    request_up_to_seq_nr: SeqNr,
    support_resend: bool,
    via_timeout: bool,
  ) -> Self {
    Self(ProducerControllerCommandKind::Request { confirmed_seq_nr, request_up_to_seq_nr, support_resend, via_timeout })
  }

  /// Creates a `Resend` command (internal, from consumer controller).
  pub(crate) const fn resend(from_seq_nr: SeqNr) -> Self {
    Self(ProducerControllerCommandKind::Resend { from_seq_nr })
  }

  /// Creates an `Ack` command (internal, from consumer controller).
  pub(crate) const fn ack(confirmed_seq_nr: SeqNr) -> Self {
    Self(ProducerControllerCommandKind::Ack { confirmed_seq_nr })
  }

  /// Creates a `ResendFirstUnconfirmed` command (internal timer).
  #[allow(dead_code)]
  pub(crate) const fn resend_first_unconfirmed() -> Self {
    Self(ProducerControllerCommandKind::ResendFirstUnconfirmed)
  }

  /// Returns a reference to the command kind.
  pub(crate) const fn kind(&self) -> &ProducerControllerCommandKind<A> {
    &self.0
  }
}
