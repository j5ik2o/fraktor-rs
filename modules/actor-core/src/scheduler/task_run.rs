//! TaskRunOnClose queue management used during scheduler shutdown.

use alloc::collections::BinaryHeap;
use core::{cmp::Ordering, fmt};

use fraktor_utils_core_rs::sync::ArcShared;

/// Priority assigned to shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRunPriority {
  /// Executed before all other tasks.
  SystemCritical,
  /// Executed after system-critical tasks.
  Runtime,
  /// Executed last.
  User,
}

impl TaskRunPriority {
  const fn rank(&self) -> u8 {
    match self {
      | Self::SystemCritical => 2,
      | Self::Runtime => 1,
      | Self::User => 0,
    }
  }
}

/// Handle returned when registering shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskRunHandle {
  id: u64,
}

impl TaskRunHandle {
  /// Creates a new handle from the provided identifier.
  #[must_use]
  pub const fn new(id: u64) -> Self {
    Self { id }
  }

  /// Returns the numeric identifier for the handle.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }
}

/// Summary returned by scheduler shutdown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskRunSummary {
  /// Number of tasks that completed successfully.
  pub executed_tasks: usize,
  /// Number of tasks that failed.
  pub failed_tasks:   usize,
}

/// Error reported by shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRunError {
  message: &'static str,
}

impl TaskRunError {
  /// Creates a new error with the provided message.
  #[must_use]
  pub const fn new(message: &'static str) -> Self {
    Self { message }
  }

  /// Returns the underlying message.
  #[must_use]
  pub const fn message(&self) -> &'static str {
    self.message
  }
}

impl fmt::Display for TaskRunError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

/// Trait implemented by shutdown tasks executed after scheduler stops accepting work.
pub trait TaskRunOnClose: Send + Sync + 'static {
  /// Executes the task.
  fn run(&self) -> Result<(), TaskRunError>;
}

#[derive(Clone)]
pub(super) struct TaskRunEntry {
  pub(super) priority: TaskRunPriority,
  pub(super) sequence: u64,
  pub(super) handle:   TaskRunHandle,
  pub(super) task:     ArcShared<dyn TaskRunOnClose>,
}

impl TaskRunEntry {
  pub(super) fn new(
    priority: TaskRunPriority,
    sequence: u64,
    handle: TaskRunHandle,
    task: ArcShared<dyn TaskRunOnClose>,
  ) -> Self {
    Self { priority, sequence, handle, task }
  }
}

impl PartialEq for TaskRunEntry {
  fn eq(&self, other: &Self) -> bool {
    self.priority.rank() == other.priority.rank() && self.sequence == other.sequence
  }
}

impl Eq for TaskRunEntry {}

impl PartialOrd for TaskRunEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for TaskRunEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.priority.rank().cmp(&other.priority.rank()) {
      | Ordering::Equal => other.sequence.cmp(&self.sequence),
      | ordering => ordering,
    }
  }
}

/// Binary heap storing registered shutdown tasks.
pub(super) type TaskRunQueue = BinaryHeap<TaskRunEntry>;
