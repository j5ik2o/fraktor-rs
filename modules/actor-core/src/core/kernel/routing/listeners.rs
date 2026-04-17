//! Composable listener registry for actors that publish events.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use super::{deafen::Deafen, listen::Listen, with_listeners::WithListeners};
use crate::core::kernel::actor::{
  actor_ref::ActorRef,
  error::SendError,
  messaging::{AnyMessage, AnyMessageView},
};

/// Stores a set of listener [`ActorRef`]s and dispatches listener-related
/// messages on behalf of a host actor.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Listeners` trait,
/// translated to Rust as a composable struct rather than a mixin. Host
/// actors embed a `Listeners` instance and delegate listener management by
/// forwarding each message through [`handle`](Self::handle):
///
/// - Returns `true` when the message was one of [`Listen`] / [`Deafen`] / [`WithListeners`] and was
///   applied by the registry.
/// - Returns `false` otherwise, so the host can route the message through its own behaviour.
///
/// Listener identity is compared by
/// [`Pid`](crate::core::kernel::actor::Pid) — re-subscribing the same `Pid`
/// does not grow the set and unsubscribing an unknown `Pid` is a no-op.
///
/// [`gossip`](Self::gossip) broadcasts a message to every registered
/// listener using the Pekko "first-error" policy: delivery is attempted
/// against all listeners and the first observed [`SendError`] is returned
/// to the caller.
#[derive(Debug, Default)]
pub struct Listeners {
  listeners: Vec<ActorRef>,
}

impl Listeners {
  /// Creates an empty listener registry.
  #[must_use]
  pub const fn new() -> Self {
    Self { listeners: Vec::new() }
  }

  /// Returns the number of currently registered listeners.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.listeners.len()
  }

  /// Returns `true` when no listeners are registered.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.listeners.is_empty()
  }

  /// Attempts to handle a listener-related message.
  ///
  /// Returns `true` when the payload was one of [`Listen`], [`Deafen`], or
  /// [`WithListeners`] and has been applied. Returns `false` otherwise so
  /// that the host actor can dispatch the message through its own logic.
  pub fn handle(&mut self, view: &AnyMessageView<'_>) -> bool {
    if let Some(listen) = view.downcast_ref::<Listen>() {
      self.add_listener(&listen.0);
      return true;
    }
    if let Some(deafen) = view.downcast_ref::<Deafen>() {
      self.remove_listener(&deafen.0);
      return true;
    }
    if let Some(with) = view.downcast_ref::<WithListeners>() {
      for listener in &self.listeners {
        with.invoke(listener);
      }
      return true;
    }
    false
  }

  /// Broadcasts `message` to every registered listener.
  ///
  /// Delivery is attempted against all listeners even when some fail. The
  /// first observed [`SendError`] is returned after every listener has been
  /// visited, matching Pekko's fan-out semantics and this project's
  /// "observable first-error" policy for multi-target sends.
  ///
  /// # Errors
  ///
  /// Returns the first [`SendError`] observed during delivery, if any.
  pub fn gossip(&mut self, message: AnyMessage) -> Result<(), SendError> {
    let mut first_error: Option<SendError> = None;
    if let Some((last, head)) = self.listeners.split_last_mut() {
      for listener in head {
        if let Err(error) = listener.try_tell(message.clone())
          && first_error.is_none()
        {
          first_error = Some(error);
        }
      }
      if let Err(error) = last.try_tell(message)
        && first_error.is_none()
      {
        first_error = Some(error);
      }
    } else {
      // No listeners registered — explicitly drop the message here so that
      // ownership is consumed on every path through the function.
      drop(message);
    }
    match first_error {
      | Some(error) => Err(error),
      | None => Ok(()),
    }
  }

  fn add_listener(&mut self, actor_ref: &ActorRef) {
    if !self.listeners.iter().any(|listener| listener.pid() == actor_ref.pid()) {
      self.listeners.push(actor_ref.clone());
    }
  }

  fn remove_listener(&mut self, actor_ref: &ActorRef) {
    if let Some(position) = self.listeners.iter().position(|listener| listener.pid() == actor_ref.pid()) {
      let _removed = self.listeners.remove(position);
    }
  }
}
