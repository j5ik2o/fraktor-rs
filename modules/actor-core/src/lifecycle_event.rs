//! Lifecycle event describing actor state transitions.

use alloc::string::String;
use core::time::Duration;

use crate::pid::Pid;

/// Lifecycle stage transitions captured for observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleStage {
  /// Actor has started.
  Started,
  /// Actor has restarted following a failure.
  Restarted,
  /// Actor has stopped.
  Stopped,
}

/// Event published whenever an actor transitions lifecycle state.
#[derive(Clone, Debug)]
pub struct LifecycleEvent {
  pid:       Pid,
  parent:    Option<Pid>,
  name:      String,
  stage:     LifecycleStage,
  timestamp: Duration,
}

impl LifecycleEvent {
  /// Creates a new lifecycle event.
  #[must_use]
  pub fn new(pid: Pid, parent: Option<Pid>, name: String, stage: LifecycleStage, timestamp: Duration) -> Self {
    Self { pid, parent, name, stage, timestamp }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the parent pid if present.
  #[must_use]
  pub const fn parent(&self) -> Option<Pid> {
    self.parent
  }

  /// Returns the logical actor name.
  #[must_use]
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Returns the lifecycle stage.
  #[must_use]
  pub const fn stage(&self) -> LifecycleStage {
    self.stage
  }

  /// Returns the event timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}
