//! Protocol responses from the backoff supervisor actor.

#[cfg(test)]
#[path = "backoff_supervisor_response_test.rs"]
mod tests;

use crate::actor::Pid;

/// Protocol responses from the backoff supervisor actor.
///
/// Corresponds to Pekko's `BackoffSupervisor.CurrentChild` and `BackoffSupervisor.RestartCount`.
#[derive(Clone, Debug)]
pub enum BackoffSupervisorResponse {
  /// The current child actor's process identifier, or `None` if no child is running.
  CurrentChild(Option<Pid>),
  /// The current restart count.
  RestartCount(u32),
}
