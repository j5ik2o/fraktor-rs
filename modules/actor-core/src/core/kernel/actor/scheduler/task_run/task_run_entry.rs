// Issue #413: TaskRunHandle, TaskRunPriority は TaskRunEntry
// のフィールド型としてのみ使用されるため同居させる。
#![allow(multiple_type_definitions)]

use alloc::boxed::Box;
use core::{
  cmp::Ordering,
  fmt::{Debug, Formatter, Result as FmtResult},
};

use fraktor_utils_core_rs::core::collections::{
  PriorityMessage,
  queue::{SyncQueue, backend::BinaryHeapPriorityBackend},
};

use crate::core::kernel::actor::scheduler::task_run::TaskRunOnClose;

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
  pub(crate) const fn rank(self) -> u8 {
    match self {
      | Self::SystemCritical => 2,
      | Self::Runtime => 1,
      | Self::User => 0,
    }
  }
}

pub(crate) struct TaskRunEntry {
  pub(crate) priority: TaskRunPriority,
  pub(crate) sequence: u64,
  pub(crate) handle:   TaskRunHandle,
  pub(crate) task:     Box<dyn TaskRunOnClose>,
}

impl Debug for TaskRunEntry {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TaskRunEntry")
      .field("priority", &self.priority)
      .field("sequence", &self.sequence)
      .field("handle", &self.handle)
      .field("task", &"<dyn TaskRunOnClose>")
      .finish()
  }
}

impl TaskRunEntry {
  pub(crate) fn new(
    priority: TaskRunPriority,
    sequence: u64,
    handle: TaskRunHandle,
    task: Box<dyn TaskRunOnClose>,
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

impl PriorityMessage for TaskRunEntry {
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority.rank() as i8)
  }
}

/// Priority queue storing registered shutdown tasks.
pub(crate) type TaskRunQueue = SyncQueue<TaskRunEntry, BinaryHeapPriorityBackend<TaskRunEntry>>;
