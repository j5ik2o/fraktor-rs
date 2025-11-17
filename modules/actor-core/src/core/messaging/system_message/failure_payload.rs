//! Failure payload snapshot for supervisor handling.

use core::time::Duration;

use super::FailureClassification;
use crate::core::{
  actor_prim::Pid,
  error::{ActorError, ActorErrorReason},
  messaging::FailureMessageSnapshot,
  supervision::RestartStatistics,
};

/// Snapshot describing a child actor failure routed through the system mailbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailurePayload {
  child:          Pid,
  reason:         ActorErrorReason,
  classification: FailureClassification,
  restart_stats:  RestartStatistics,
  message:        Option<FailureMessageSnapshot>,
  timestamp:      Duration,
}

impl FailurePayload {
  /// Creates a payload from the provided error and context.
  #[must_use]
  pub fn from_error(
    child: Pid,
    error: &ActorError,
    message: Option<FailureMessageSnapshot>,
    timestamp: Duration,
  ) -> Self {
    Self {
      child,
      reason: error.reason().clone(),
      classification: FailureClassification::from(error),
      restart_stats: RestartStatistics::new(),
      message,
      timestamp,
    }
  }

  /// Replaces the restart statistics snapshot embedded in the payload.
  #[must_use]
  pub fn with_restart_stats(mut self, stats: RestartStatistics) -> Self {
    self.restart_stats = stats;
    self
  }

  /// Returns the failed child pid.
  #[must_use]
  pub const fn child(&self) -> Pid {
    self.child
  }

  /// Returns the cloned reason associated with the failure.
  #[must_use]
  pub const fn reason(&self) -> &ActorErrorReason {
    &self.reason
  }

  /// Returns whether the failure was fatal or recoverable.
  #[must_use]
  pub const fn classification(&self) -> FailureClassification {
    self.classification
  }

  /// Returns the recorded timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }

  /// Returns the restart statistics snapshot.
  #[must_use]
  pub const fn restart_stats(&self) -> &RestartStatistics {
    &self.restart_stats
  }

  /// Returns the captured message snapshot, if any.
  #[must_use]
  pub const fn message(&self) -> Option<&FailureMessageSnapshot> {
    self.message.as_ref()
  }

  /// Converts the payload into an [`ActorError`] using the stored reason/classification.
  #[must_use]
  pub fn to_actor_error(&self) -> ActorError {
    match self.classification {
      | FailureClassification::Recoverable => ActorError::recoverable(self.reason.clone()),
      | FailureClassification::Fatal => ActorError::fatal(self.reason.clone()),
    }
  }

  /// Consumes the payload and returns the embedded snapshot (if any).
  #[must_use]
  pub fn into_message_snapshot(self) -> Option<FailureMessageSnapshot> {
    self.message
  }
}
