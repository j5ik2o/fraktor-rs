//! Message adapter between state-sourced effector signals and aggregate messages.

#[cfg(test)]
#[path = "state_sourced_effector_message_adapter_test.rs"]
mod tests;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::StateSourcedEffectorSignal;

type WrapSignal<S, M> = dyn Fn(StateSourcedEffectorSignal<S>) -> M + Send + Sync;
type UnwrapSignal<S, M> = dyn for<'a> Fn(&'a M) -> Option<&'a StateSourcedEffectorSignal<S>> + Send + Sync;

/// Converts public state-sourced effector signals to and from an actor-private message type.
pub struct StateSourcedEffectorMessageAdapter<S, M> {
  wrap_signal:   ArcShared<WrapSignal<S, M>>,
  unwrap_signal: ArcShared<UnwrapSignal<S, M>>,
}

impl<S, M> StateSourcedEffectorMessageAdapter<S, M> {
  /// Creates a message adapter from wrapping and unwrapping functions.
  #[must_use]
  pub fn new<Wrap, Unwrap>(wrap_signal: Wrap, unwrap_signal: Unwrap) -> Self
  where
    Wrap: Fn(StateSourcedEffectorSignal<S>) -> M + Send + Sync + 'static,
    Unwrap: for<'a> Fn(&'a M) -> Option<&'a StateSourcedEffectorSignal<S>> + Send + Sync + 'static, {
    Self { wrap_signal: ArcShared::new(wrap_signal), unwrap_signal: ArcShared::new(unwrap_signal) }
  }

  /// Wraps a state-sourced effector signal into an aggregate message.
  #[must_use]
  pub fn wrap_signal(&self, signal: StateSourcedEffectorSignal<S>) -> M {
    (self.wrap_signal)(signal)
  }

  /// Borrows a state-sourced effector signal from an aggregate message.
  #[must_use]
  pub fn unwrap_signal<'a>(&self, message: &'a M) -> Option<&'a StateSourcedEffectorSignal<S>> {
    (self.unwrap_signal)(message)
  }
}

impl<S, M> Clone for StateSourcedEffectorMessageAdapter<S, M> {
  fn clone(&self) -> Self {
    Self { wrap_signal: self.wrap_signal.clone(), unwrap_signal: self.unwrap_signal.clone() }
  }
}
