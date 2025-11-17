use core::cmp::Ordering;

use fraktor_utils_core_rs::core::{
  collections::{
    PriorityMessage,
    queue::{SyncPriorityQueue, backend::BinaryHeapPriorityBackend},
  },
  sync::ArcShared,
};

use crate::scheduler::{TaskRunHandle, TaskRunOnClose, TaskRunPriority};

#[derive(Clone)]
pub(crate) struct TaskRunEntry {
  pub(crate) priority: TaskRunPriority,
  pub(crate) sequence: u64,
  pub(crate) handle:   TaskRunHandle,
  pub(crate) task:     ArcShared<dyn TaskRunOnClose>,
}

impl core::fmt::Debug for TaskRunEntry {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

impl PriorityMessage for TaskRunEntry {
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority.rank() as i8)
  }
}

/// Priority queue storing registered shutdown tasks.
pub(crate) type TaskRunQueue = SyncPriorityQueue<TaskRunEntry, BinaryHeapPriorityBackend<TaskRunEntry>>;
