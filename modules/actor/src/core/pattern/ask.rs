//! Timeout-aware ask helpers.

use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::{
  actor::actor_ref::ActorRef,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskError, AskResponse, AskResult},
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::state::SystemStateShared,
};

/// Sends a request and arranges timeout completion on the returned ask future.
///
/// The returned future resolves with `Err(AskError::Timeout)` when the
/// request cannot be observed before the deadline.
#[must_use]
pub fn ask_with_timeout(actor_ref: &mut ActorRef, message: AnyMessage, timeout: Duration) -> AskResponse {
  let ask_response = actor_ref.ask(message);
  if ask_response.future().with_read(|inner| inner.is_ready()) {
    return ask_response;
  }
  if let Some(system) = actor_ref.system_state() {
    install_ask_timeout(ask_response.future(), &system, timeout);
  } else {
    complete_with_timeout(ask_response.future());
  }
  ask_response
}

pub(crate) fn install_ask_timeout(
  future: &ActorFutureShared<AskResult>,
  system: &SystemStateShared,
  timeout: Duration,
) {
  if future.with_read(|inner| inner.is_ready()) {
    return;
  }
  if timeout.is_zero() {
    complete_with_timeout(future);
    return;
  }

  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(AskTimeoutRunnable { future: future.clone() });
  let result = system.scheduler().with_write(|scheduler| {
    scheduler
      .schedule_command(timeout, SchedulerCommand::RunRunnable { runnable: runnable.clone(), dispatcher: None })
  });
  if result.is_err() {
    complete_with_timeout(future);
  }
}

fn complete_with_timeout(future: &ActorFutureShared<AskResult>) {
  if future.with_read(|inner| inner.is_ready()) {
    return;
  }
  let waker = future.with_write(|inner| inner.complete(Err(AskError::Timeout)));
  if let Some(waker) = waker {
    waker.wake();
  }
}

struct AskTimeoutRunnable {
  future: ActorFutureShared<AskResult>,
}

impl SchedulerRunnable for AskTimeoutRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    complete_with_timeout(&self.future);
  }
}
