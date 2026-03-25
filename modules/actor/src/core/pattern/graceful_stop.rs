//! Graceful stop helper.

use core::{cmp, time::Duration};

use fraktor_utils_rs::core::timing::delay::{DelayFuture, DelayProvider};

use crate::core::{
  actor::actor_ref::ActorRef,
  messaging::{AnyMessage, AskError, system_message::SystemMessage},
};

const STOP_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Sends `PoisonPill` and waits until the target actor disappears from the system registry.
///
/// # Errors
///
/// Returns [`AskError::SendFailed`] when the system state is unavailable,
/// or [`AskError::Timeout`] when the actor does not stop before `timeout`.
pub async fn graceful_stop(target: &ActorRef, timeout: Duration) -> Result<(), AskError> {
  graceful_stop_with_message(target, AnyMessage::new(SystemMessage::PoisonPill), timeout).await
}

/// Sends the supplied stop message and waits until the target actor disappears from the system
/// registry.
///
/// # Errors
///
/// Returns [`AskError::SendFailed`] when the system state is unavailable,
/// or [`AskError::Timeout`] when the actor does not stop before `timeout`.
pub async fn graceful_stop_with_message(
  target: &ActorRef,
  stop_message: AnyMessage,
  timeout: Duration,
) -> Result<(), AskError> {
  let Some(system) = target.system_state() else {
    return Err(AskError::SendFailed);
  };
  let pid = target.pid();
  if system.cell(&pid).is_none() {
    return Ok(());
  }
  target.try_tell(stop_message).map_err(|_| AskError::SendFailed)?;

  let mut remaining = timeout;
  let mut delay_provider = system.delay_provider();
  loop {
    if system.cell(&pid).is_none() {
      return Ok(());
    }
    if remaining.is_zero() {
      return Err(AskError::Timeout);
    }
    let delay_duration = cmp::min(remaining, STOP_POLL_INTERVAL);
    let delay: DelayFuture = delay_provider.delay(delay_duration);
    delay.await;
    remaining = remaining.saturating_sub(delay_duration);
  }
}
