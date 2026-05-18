//! Common dispatcher state shared by every concrete `MessageDispatcher`.
//!
//! `DispatcherCore` carries the configuration, executor, and lifecycle state
//! that is identical across `DefaultDispatcher`, `PinnedDispatcher`, and
//! `BalancingDispatcher`. The struct is intentionally CQS-strict and free of
//! interior mutability: the surrounding `MessageDispatcherShared` provides the
//! `SpinSyncMutex` so callers can run command methods through `&mut self`.

#[cfg(test)]
#[path = "dispatcher_core_test.rs"]
mod tests;

use alloc::string::{String, ToString};
use core::{num::NonZeroUsize, time::Duration};

use super::{
  dispatcher_config::DispatcherConfig, executor_shared::ExecutorShared, shutdown_schedule::ShutdownSchedule,
};

/// Identifier-keyed dispatcher state shared by concrete dispatcher types.
pub struct DispatcherCore {
  id:                  String,
  throughput:          NonZeroUsize,
  throughput_deadline: Option<Duration>,
  shutdown_timeout:    Duration,
  executor:            ExecutorShared,
  inhabitants:         i64,
  shutdown_schedule:   ShutdownSchedule,
}

impl DispatcherCore {
  /// Constructs the core from an immutable settings bundle and an executor.
  #[must_use]
  pub fn new(settings: &DispatcherConfig, executor: ExecutorShared) -> Self {
    Self {
      id: settings.id().to_string(),
      throughput: settings.throughput(),
      throughput_deadline: settings.throughput_deadline(),
      shutdown_timeout: settings.shutdown_timeout(),
      executor,
      inhabitants: 0,
      shutdown_schedule: ShutdownSchedule::Unscheduled,
    }
  }

  /// Returns the dispatcher identifier.
  #[must_use]
  pub fn id(&self) -> &str {
    &self.id
  }

  /// Returns the configured throughput.
  #[must_use]
  pub const fn throughput(&self) -> NonZeroUsize {
    self.throughput
  }

  /// Returns the configured throughput deadline.
  #[must_use]
  pub const fn throughput_deadline(&self) -> Option<Duration> {
    self.throughput_deadline
  }

  /// Returns the configured shutdown timeout.
  #[must_use]
  pub const fn shutdown_timeout(&self) -> Duration {
    self.shutdown_timeout
  }

  /// Returns the current inhabitants count (number of attached actors).
  #[must_use]
  pub const fn inhabitants(&self) -> i64 {
    self.inhabitants
  }

  /// Returns a borrow of the underlying `ExecutorShared`.
  #[must_use]
  pub const fn executor(&self) -> &ExecutorShared {
    &self.executor
  }

  /// Returns the current `ShutdownSchedule` state.
  #[must_use]
  pub const fn shutdown_schedule(&self) -> ShutdownSchedule {
    self.shutdown_schedule
  }

  /// Increments the inhabitants counter and cancels a pending delayed shutdown.
  ///
  /// If a delayed shutdown was already scheduled, it transitions to
  /// `Rescheduled`, telling the eventual delayed-fire callback to skip the
  /// shutdown.
  pub const fn mark_attach(&mut self) {
    self.inhabitants += 1;
    if matches!(self.shutdown_schedule, ShutdownSchedule::Scheduled) {
      self.shutdown_schedule = ShutdownSchedule::Rescheduled;
    }
  }

  /// Decrements the inhabitants counter.
  ///
  /// In debug builds an underflow triggers a `debug_assert!` to surface
  /// register/unregister mismatches loudly. In release builds the counter is
  /// clamped to zero and an error log is emitted to preserve observability.
  pub fn mark_detach(&mut self) {
    debug_assert!(
      self.inhabitants > 0,
      "DispatcherCore::mark_detach underflow: inhabitants={} (id={})",
      self.inhabitants,
      self.id
    );
    if self.inhabitants > 0 {
      self.inhabitants -= 1;
    } else {
      tracing::error!(
        target: "fraktor::dispatcher",
        dispatcher_id = %self.id,
        "DispatcherCore::mark_detach observed zero inhabitants; clamping to zero"
      );
      self.inhabitants = 0;
    }
  }

  /// Transitions the shutdown schedule when the dispatcher has no inhabitants.
  ///
  /// Returns the post-transition [`ShutdownSchedule`] so the caller can copy
  /// the value out of the lock and decide whether to register a delayed
  /// shutdown closure without re-acquiring the mutex.
  pub const fn schedule_shutdown_if_sensible(&mut self) -> ShutdownSchedule {
    if self.inhabitants == 0 {
      self.shutdown_schedule = match self.shutdown_schedule {
        | ShutdownSchedule::Unscheduled => ShutdownSchedule::Scheduled,
        | ShutdownSchedule::Scheduled | ShutdownSchedule::Rescheduled => ShutdownSchedule::Rescheduled,
      };
    }
    self.shutdown_schedule
  }

  /// Shuts the underlying executor down and resets the schedule state.
  pub fn shutdown(&mut self) {
    self.executor.shutdown();
    self.shutdown_schedule = ShutdownSchedule::Unscheduled;
  }
}
