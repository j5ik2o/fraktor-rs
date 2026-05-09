//! Intermediate receive builder for typed behaviors.
//!
//! Corresponds to Pekko's `Receive[T]` from
//! `org.apache.pekko.actor.typed.scaladsl.Behaviors`. Wraps a [`Behavior`]
//! built from a message handler, allowing a signal handler to be chained
//! before the result is used as a full [`Behavior`].

#[cfg(test)]
mod tests;

use fraktor_actor_core_kernel_rs::actor::error::ActorError;

use crate::{actor::TypedActorContext, behavior::Behavior, message_and_signals::BehaviorSignal};

/// Intermediate builder produced by [`Behaviors::receive`].
///
/// Holds a message-only [`Behavior`] and allows chaining a signal handler
/// via [`receive_signal`](Self::receive_signal). Converts to a full
/// [`Behavior`] either through the chain or via [`From`].
///
/// Corresponds to Pekko's `Receive[T]`.
pub struct Receive<M>
where
  M: Send + Sync + 'static, {
  inner: Behavior<M>,
}

impl<M> Receive<M>
where
  M: Send + Sync + 'static,
{
  /// Creates a new receive builder wrapping the given behavior.
  pub(crate) const fn new(inner: Behavior<M>) -> Self {
    Self { inner }
  }

  /// Attaches a signal handler and produces the final [`Behavior`].
  ///
  /// Corresponds to Pekko's `Receive[T].receiveSignal`.
  pub fn receive_signal<F>(self, handler: F) -> Behavior<M>
  where
    F: for<'a> Fn(&mut TypedActorContext<'a, M>, &BehaviorSignal) -> Result<Behavior<M>, ActorError>
      + Send
      + Sync
      + 'static, {
    self.inner.receive_signal(handler)
  }
}

impl<M> From<Receive<M>> for Behavior<M>
where
  M: Send + Sync + 'static,
{
  fn from(receive: Receive<M>) -> Self {
    receive.inner
  }
}
