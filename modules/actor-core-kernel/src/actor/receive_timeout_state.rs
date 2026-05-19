//! Runtime state backing `ActorContext` receive-timeout scheduling.

use core::time::Duration;

use crate::actor::{messaging::AnyMessage, scheduler::SchedulerHandle};

/// Runtime state backing `ActorContext` receive-timeout scheduling.
pub(crate) struct ReceiveTimeoutState {
  pub(crate) duration:            Duration,
  pub(crate) message:             AnyMessage,
  pub(crate) handle:              Option<SchedulerHandle>,
  /// Monotonically-increasing counter of `schedule_receive_timeout`
  /// invocations. Incremented on every `set_receive_timeout` /
  /// `reschedule_receive_timeout` run, read by
  /// [`ActorContext::receive_timeout_schedule_generation`](crate::actor::ActorContext::receive_timeout_schedule_generation)
  /// for diagnostics and Pekko `NotInfluenceReceiveTimeout` observation.
  pub(crate) schedule_generation: u64,
}

impl ReceiveTimeoutState {
  /// Creates receive-timeout state for an armed timeout message.
  #[must_use]
  pub(crate) const fn new(duration: Duration, message: AnyMessage) -> Self {
    Self { duration, message, handle: None, schedule_generation: 0 }
  }

  /// Returns the monotonic schedule-generation counter.
  ///
  /// Incremented once per `schedule_receive_timeout` invocation (both the
  /// initial `set_receive_timeout` arm and every
  /// `reschedule_receive_timeout` cancel+schedule).
  #[must_use]
  pub(crate) const fn schedule_generation(&self) -> u64 {
    self.schedule_generation
  }
}
