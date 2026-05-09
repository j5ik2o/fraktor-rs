//! Signal delivered when a watched child fails.

use core::convert::Infallible;

use fraktor_actor_core_kernel_rs::actor::{Pid, error::ActorError};

use crate::{
  TypedActorRef,
  message_and_signals::{BehaviorSignal, Signal, Terminated},
};

/// Public signal emitted when a child actor fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildFailed {
  terminated: Terminated,
  error:      ActorError,
}

impl ChildFailed {
  /// Creates a new child-failed signal payload.
  #[must_use]
  pub const fn new(actor_ref: TypedActorRef<Infallible>, error: ActorError) -> Self {
    Self { terminated: Terminated::new(actor_ref), error }
  }

  /// Returns the terminated child signal contract shared with [`Terminated`].
  #[must_use]
  pub const fn terminated(&self) -> &Terminated {
    &self.terminated
  }

  /// Returns the failed child actor reference.
  #[must_use]
  pub const fn actor_ref(&self) -> &TypedActorRef<Infallible> {
    self.terminated.actor_ref()
  }

  /// Returns the failed child pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.actor_ref().pid()
  }

  /// Returns the error that caused the child to fail.
  #[must_use]
  pub const fn error(&self) -> &ActorError {
    &self.error
  }
}

impl Signal for ChildFailed {}

impl From<ChildFailed> for BehaviorSignal {
  fn from(value: ChildFailed) -> Self {
    Self::ChildFailed(value)
  }
}
