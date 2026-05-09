//! Signal delivered when a watched actor terminates.

use core::convert::Infallible;

use fraktor_actor_core_rs::actor::Pid;

use crate::{
  TypedActorRef,
  message_and_signals::{BehaviorSignal, Signal},
};

/// Public signal emitted when a watched actor terminates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminated {
  actor_ref: TypedActorRef<Infallible>,
}

impl Terminated {
  /// Creates a new terminated signal payload.
  #[must_use]
  pub const fn new(actor_ref: TypedActorRef<Infallible>) -> Self {
    Self { actor_ref }
  }

  /// Returns the terminated actor reference.
  #[must_use]
  pub const fn actor_ref(&self) -> &TypedActorRef<Infallible> {
    &self.actor_ref
  }

  /// Returns the terminated actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.actor_ref.pid()
  }
}

impl Signal for Terminated {}

impl From<Terminated> for BehaviorSignal {
  fn from(value: Terminated) -> Self {
    Self::Terminated(value)
  }
}
