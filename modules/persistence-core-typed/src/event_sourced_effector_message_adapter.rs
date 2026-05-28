//! Message adapter between effector signals and aggregate messages.

use fraktor_utils_core_rs::sync::ArcShared;

use crate::EventSourcedEffectorSignal;

type WrapSignal<S, E, M> = dyn Fn(EventSourcedEffectorSignal<S, E>) -> M + Send + Sync;
type UnwrapSignal<S, E, M> = dyn for<'a> Fn(&'a M) -> Option<&'a EventSourcedEffectorSignal<S, E>> + Send + Sync;

/// Converts public effector signals to and from an actor-private message type.
pub struct EventSourcedEffectorMessageAdapter<S, E, M> {
  wrap_signal:   ArcShared<WrapSignal<S, E, M>>,
  unwrap_signal: ArcShared<UnwrapSignal<S, E, M>>,
}

impl<S, E, M> EventSourcedEffectorMessageAdapter<S, E, M> {
  /// Creates a message adapter from wrapping and unwrapping functions.
  #[must_use]
  pub fn new<Wrap, Unwrap>(wrap_signal: Wrap, unwrap_signal: Unwrap) -> Self
  where
    Wrap: Fn(EventSourcedEffectorSignal<S, E>) -> M + Send + Sync + 'static,
    Unwrap: for<'a> Fn(&'a M) -> Option<&'a EventSourcedEffectorSignal<S, E>> + Send + Sync + 'static, {
    Self { wrap_signal: ArcShared::new(wrap_signal), unwrap_signal: ArcShared::new(unwrap_signal) }
  }

  /// Wraps an event-sourced effector signal into an aggregate message.
  #[must_use]
  pub fn wrap_signal(&self, signal: EventSourcedEffectorSignal<S, E>) -> M {
    (self.wrap_signal)(signal)
  }

  /// Borrows an event-sourced effector signal from an aggregate message.
  #[must_use]
  pub fn unwrap_signal<'a>(&self, message: &'a M) -> Option<&'a EventSourcedEffectorSignal<S, E>> {
    (self.unwrap_signal)(message)
  }
}

impl<S, E, M> Clone for EventSourcedEffectorMessageAdapter<S, E, M> {
  fn clone(&self) -> Self {
    Self { wrap_signal: self.wrap_signal.clone(), unwrap_signal: self.unwrap_signal.clone() }
  }
}
