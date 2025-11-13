use alloc::collections::BinaryHeap;
use core::cmp::Ordering;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::scheduler::{TaskRunHandle, TaskRunOnClose, TaskRunPriority};

#[derive(Clone)]
pub(crate) struct TaskRunEntry {
  pub(crate) priority: TaskRunPriority,
  pub(crate) sequence: u64,
  pub(crate) handle:   TaskRunHandle,
  pub(crate) task:     ArcShared<dyn TaskRunOnClose>,
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

/// Binary heap storing registered shutdown tasks.
pub(crate) type TaskRunQueue = BinaryHeap<TaskRunEntry>;
