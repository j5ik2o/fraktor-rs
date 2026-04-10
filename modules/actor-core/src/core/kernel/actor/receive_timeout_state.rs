//! Runtime state backing `ActorContext` receive-timeout scheduling.

use core::time::Duration;

use crate::core::kernel::actor::{messaging::AnyMessage, scheduler::SchedulerHandle};

/// Runtime state backing `ActorContext` receive-timeout scheduling.
pub(crate) struct ReceiveTimeoutState {
  pub(crate) duration: Duration,
  pub(crate) message:  AnyMessage,
  pub(crate) handle:   Option<SchedulerHandle>,
}

impl ReceiveTimeoutState {
  #[must_use]
  pub(crate) const fn new(duration: Duration, message: AnyMessage) -> Self {
    Self { duration, message, handle: None }
  }
}
