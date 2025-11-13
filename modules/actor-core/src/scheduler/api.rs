//! Canonical scheduler APIs exposed to actor subsystems.

use core::time::Duration;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  RuntimeToolbox,
  actor_prim::actor_ref::ActorRefGeneric,
  messaging::AnyMessageGeneric,
  scheduler::{
    command::SchedulerCommand, dispatcher_sender_shared::DispatcherSenderShared, error::SchedulerError,
    handle::SchedulerHandle, runnable::SchedulerRunnable, scheduler_core::Scheduler,
  },
};

/// Schedules a one-shot message delivery matching Pekko's `scheduleOnce` semantics.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
///
/// # Examples
/// ```rust,no_run
/// # use core::time::Duration;
/// # use fraktor_actor_core_rs::{
/// #   actor_prim::actor_ref::ActorRefGeneric,
/// #   messaging::AnyMessageGeneric,
/// #   scheduler::{api, Scheduler, SchedulerConfig},
/// # };
/// # use fraktor_utils_core_rs::runtime_toolbox::NoStdToolbox;
/// # fn main() {
/// let mut scheduler = Scheduler::new(NoStdToolbox::default(), SchedulerConfig::default());
/// let receiver = ActorRefGeneric::null();
/// api::schedule_once(
///   &mut scheduler,
///   Duration::from_millis(5),
///   receiver,
///   AnyMessageGeneric::new(7u32),
///   None,
///   None,
/// )
/// .unwrap();
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn schedule_once<TB: RuntimeToolbox>(
  scheduler: &mut Scheduler<TB>,
  delay: Duration,
  receiver: ActorRefGeneric<TB>,
  message: AnyMessageGeneric<TB>,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  sender: Option<ActorRefGeneric<TB>>,
) -> Result<SchedulerHandle, SchedulerError> {
  scheduler.schedule_command(delay, build_send_message_command(receiver, message, dispatcher, sender))
}

/// Schedules a fixed-rate message delivery.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay or interval is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
///
/// # Examples
/// ```rust,no_run
/// # use core::time::Duration;
/// # use fraktor_actor_core_rs::{
/// #   actor_prim::actor_ref::ActorRefGeneric,
/// #   messaging::AnyMessageGeneric,
/// #   scheduler::{api, Scheduler, SchedulerConfig},
/// # };
/// # use fraktor_utils_core_rs::runtime_toolbox::NoStdToolbox;
/// # fn main() {
/// let mut scheduler = Scheduler::new(NoStdToolbox::default(), SchedulerConfig::default());
/// let receiver = ActorRefGeneric::null();
/// api::schedule_at_fixed_rate(
///   &mut scheduler,
///   Duration::from_millis(2),
///   Duration::from_millis(4),
///   receiver,
///   AnyMessageGeneric::new(11u32),
///   None,
///   None,
/// )
/// .unwrap();
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn schedule_at_fixed_rate<TB: RuntimeToolbox>(
  scheduler: &mut Scheduler<TB>,
  initial_delay: Duration,
  interval: Duration,
  receiver: ActorRefGeneric<TB>,
  message: AnyMessageGeneric<TB>,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  sender: Option<ActorRefGeneric<TB>>,
) -> Result<SchedulerHandle, SchedulerError> {
  let command = build_send_message_command(receiver, message, dispatcher, sender);
  scheduler.schedule_at_fixed_rate_with_command(initial_delay, interval, command)
}

/// Schedules a fixed-rate runnable task.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay or interval is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
pub fn schedule_at_fixed_rate_fn<TB, F>(
  scheduler: &mut Scheduler<TB>,
  initial_delay: Duration,
  interval: Duration,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  runnable: F,
) -> Result<SchedulerHandle, SchedulerError>
where
  TB: RuntimeToolbox,
  F: SchedulerRunnable, {
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
  scheduler.schedule_at_fixed_rate_with_command(initial_delay, interval, SchedulerCommand::RunRunnable {
    runnable,
    dispatcher,
  })
}

/// Schedules a fixed-delay message delivery.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
///
/// # Examples
/// ```rust,no_run
/// # use core::time::Duration;
/// # use fraktor_actor_core_rs::{
/// #   actor_prim::actor_ref::ActorRefGeneric,
/// #   messaging::AnyMessageGeneric,
/// #   scheduler::{api, Scheduler, SchedulerConfig},
/// # };
/// # use fraktor_utils_core_rs::runtime_toolbox::NoStdToolbox;
/// # fn main() {
/// let mut scheduler = Scheduler::new(NoStdToolbox::default(), SchedulerConfig::default());
/// let receiver = ActorRefGeneric::null();
/// api::schedule_with_fixed_delay(
///   &mut scheduler,
///   Duration::from_millis(1),
///   Duration::from_millis(3),
///   receiver,
///   AnyMessageGeneric::new(5u32),
///   None,
///   None,
/// )
/// .unwrap();
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn schedule_with_fixed_delay<TB: RuntimeToolbox>(
  scheduler: &mut Scheduler<TB>,
  initial_delay: Duration,
  delay: Duration,
  receiver: ActorRefGeneric<TB>,
  message: AnyMessageGeneric<TB>,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  sender: Option<ActorRefGeneric<TB>>,
) -> Result<SchedulerHandle, SchedulerError> {
  let command = build_send_message_command(receiver, message, dispatcher, sender);
  scheduler.schedule_with_fixed_delay_with_command(initial_delay, delay, command)
}

/// Schedules a fixed-delay runnable task.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
pub fn schedule_with_fixed_delay_fn<TB, F>(
  scheduler: &mut Scheduler<TB>,
  initial_delay: Duration,
  delay: Duration,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  runnable: F,
) -> Result<SchedulerHandle, SchedulerError>
where
  TB: RuntimeToolbox,
  F: SchedulerRunnable, {
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
  scheduler.schedule_with_fixed_delay_with_command(initial_delay, delay, SchedulerCommand::RunRunnable {
    runnable,
    dispatcher,
  })
}

/// Schedules a runnable task for one-shot execution.
///
/// # Errors
///
/// Returns [`SchedulerError::InvalidDelay`] if the delay is zero or negative.
/// Returns [`SchedulerError::Backpressured`] if the scheduler is at capacity.
/// Returns [`SchedulerError::Closed`] if the scheduler has been shut down.
///
/// # Examples
/// ```rust,no_run
/// # use core::time::Duration;
/// # use core::sync::atomic::{AtomicUsize, Ordering};
/// # use fraktor_actor_core_rs::scheduler::{api, ExecutionBatch, Scheduler, SchedulerConfig};
/// # use fraktor_utils_core_rs::runtime_toolbox::NoStdToolbox;
/// # fn main() {
/// let mut scheduler = Scheduler::new(NoStdToolbox::default(), SchedulerConfig::default());
/// let counter = AtomicUsize::new(0);
/// api::schedule_once_fn(
///   &mut scheduler,
///   Duration::from_millis(1),
///   None,
///   move |_batch: &ExecutionBatch| {
///     counter.fetch_add(1, Ordering::Relaxed);
///   },
/// )
/// .unwrap();
/// # }
/// ```
pub fn schedule_once_fn<TB, F>(
  scheduler: &mut Scheduler<TB>,
  delay: Duration,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  runnable: F,
) -> Result<SchedulerHandle, SchedulerError>
where
  TB: RuntimeToolbox,
  F: SchedulerRunnable, {
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
  scheduler.schedule_command(delay, SchedulerCommand::RunRunnable { runnable, dispatcher })
}

#[allow(clippy::missing_const_for_fn)]
fn build_send_message_command<TB: RuntimeToolbox>(
  receiver: ActorRefGeneric<TB>,
  message: AnyMessageGeneric<TB>,
  dispatcher: Option<DispatcherSenderShared<TB>>,
  sender: Option<ActorRefGeneric<TB>>,
) -> SchedulerCommand<TB> {
  SchedulerCommand::SendMessage { receiver, message, dispatcher, sender }
}
