//! Runtime state backing `ActorContext` receive-timeout scheduling.

use core::time::Duration;

use crate::core::kernel::actor::{messaging::AnyMessage, scheduler::SchedulerHandle};

/// Runtime state backing `ActorContext` receive-timeout scheduling.
pub struct ReceiveTimeoutState {
  pub(crate) duration: Duration,
  pub(crate) message:  AnyMessage,
  pub(crate) handle:   Option<SchedulerHandle>,
}

impl ReceiveTimeoutState {
  /// Creates receive-timeout state for an armed timeout message.
  #[must_use]
  pub const fn new(duration: Duration, message: AnyMessage) -> Self {
    Self { duration, message, handle: None }
  }
}
