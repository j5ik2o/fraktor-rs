//! Timeout-aware ask helpers.

use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

use crate::core::{
  actor::actor_ref::ActorRef,
  error::SendError,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskError, AskResponse, AskResult},
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::state::SystemStateShared,
};

/// Sends a request and arranges timeout completion on the returned ask future.
///
/// # Errors
///
/// Returns an error if the request cannot be delivered.
pub fn ask_with_timeout(
  actor_ref: &ActorRef,
  message: AnyMessage,
  timeout: Duration,
) -> Result<AskResponse, SendError> {
  let response = actor_ref.ask(message)?;
  if let Some(system) = actor_ref.system_state() {
    install_ask_timeout(response.future(), &system, timeout);
  } else {
    complete_with_timeout(response.future());
  }
  Ok(response)
}

pub(crate) fn install_ask_timeout(
  future: &ActorFutureShared<AskResult>,
  system: &SystemStateShared,
  timeout: Duration,
) {
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
