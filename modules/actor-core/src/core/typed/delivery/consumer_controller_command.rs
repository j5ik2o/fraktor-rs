//! Commands accepted by the consumer controller actor.

#[cfg(test)]
mod tests;

use crate::core::typed::{
  TypedActorRef,
  delivery::{ConsumerControllerDelivery, ProducerControllerCommand, SequencedMessage},
};

/// Commands handled by
/// [`ConsumerController`](crate::core::typed::delivery::ConsumerController).
///
/// User code constructs commands through
/// [`ConsumerController`](crate::core::typed::delivery::ConsumerController)
/// factory methods. Internal protocol messages are crate-private.
#[derive(Clone)]
pub struct ConsumerControllerCommand<A>(pub(crate) ConsumerControllerCommandKind<A>)
where
  A: Clone + Send + Sync + 'static;

#[derive(Clone)]
pub(crate) enum ConsumerControllerCommandKind<A>
where
  A: Clone + Send + Sync + 'static, {
  /// Initial message from the consumer actor.
  Start { deliver_to: TypedActorRef<ConsumerControllerDelivery<A>> },
  /// Register this consumer controller to a producer controller.
  RegisterToProducerController { producer_controller: TypedActorRef<ProducerControllerCommand<A>> },
  /// A sequenced message from the producer controller (wire protocol).
  SequencedMsg(SequencedMessage<A>),
  /// Confirmation from the consumer that it processed the delivered message.
  Confirmed,
  /// Stop after delivering all remaining messages.
  DeliverThenStop,
  /// Internal retry timer.
  #[allow(dead_code)]
  Retry,
  /// Internal: consumer actor terminated.
  #[allow(dead_code)]
  ConsumerTerminated,
}

impl<A> ConsumerControllerCommand<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a `Start` command.
  pub(crate) const fn start(deliver_to: TypedActorRef<ConsumerControllerDelivery<A>>) -> Self {
    Self(ConsumerControllerCommandKind::Start { deliver_to })
  }

  /// Creates a `RegisterToProducerController` command.
  pub(crate) const fn register_to_producer_controller(
    producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
  ) -> Self {
    Self(ConsumerControllerCommandKind::RegisterToProducerController { producer_controller })
  }

  /// Creates a `SequencedMsg` command (internal, from producer controller).
  pub(crate) const fn sequenced_msg(msg: SequencedMessage<A>) -> Self {
    Self(ConsumerControllerCommandKind::SequencedMsg(msg))
  }

  /// Creates a `Confirmed` command.
  pub(crate) const fn confirmed() -> Self {
    Self(ConsumerControllerCommandKind::Confirmed)
  }

  /// Creates a `DeliverThenStop` command.
  pub(crate) const fn deliver_then_stop() -> Self {
    Self(ConsumerControllerCommandKind::DeliverThenStop)
  }

  /// Returns a reference to the command kind.
  pub(crate) const fn kind(&self) -> &ConsumerControllerCommandKind<A> {
    &self.0
  }
}
