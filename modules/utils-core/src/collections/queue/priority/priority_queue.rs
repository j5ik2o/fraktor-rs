use alloc::vec::Vec;
use core::marker::PhantomData;

use super::priority_message::PriorityMessage;
use crate::collections::queue::{
  queue_error::QueueError,
  queue_size::QueueSize,
  traits::{QueueBase, QueueReader, QueueRw, QueueWriter},
};

/// Priority queue facade backed by multiple level queues.
#[derive(Debug, Clone)]
pub struct PriorityQueue<Q, E>
where
  Q: QueueRw<E>, {
  levels:  Vec<Q>,
  _marker: PhantomData<E>,
}

impl<Q, E> PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
{
  /// Creates a new priority queue.
  #[must_use]
  pub fn new(levels: Vec<Q>) -> Self {
    assert!(!levels.is_empty(), "PriorityQueue requires at least one level");
    Self { levels, _marker: PhantomData }
  }

  /// Immutable references to queues at each level.
  #[must_use]
  pub fn levels(&self) -> &[Q] {
    &self.levels
  }

  /// Mutable references to queues at each level.
  pub fn levels_mut(&mut self) -> &mut [Q] {
    &mut self.levels
  }

  fn level_index(&self, priority: Option<i8>) -> usize {
    let levels = self.levels.len();
    let default = (levels / 2) as i8;
    let max = (levels as i32 - 1) as i8;
    priority.unwrap_or(default).clamp(0, max) as usize
  }

  /// Adds an element to the queue based on its priority.
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: PriorityMessage, {
    let idx = self.level_index(element.get_priority());
    self.levels[idx].offer(element)
  }

  /// Removes an element from the queue, preferring higher priorities.
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: PriorityMessage, {
    for queue in self.levels.iter().rev() {
      match queue.poll()? {
        | Some(item) => return Ok(Some(item)),
        | None => continue,
      }
    }
    Ok(None)
  }

  /// Cleans up queues at all levels.
  pub fn clean_up(&self) {
    for queue in &self.levels {
      queue.clean_up();
    }
  }

  fn aggregate_len(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.len() {
        | QueueSize::Limitless => return QueueSize::limitless(),
        | QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }

  fn aggregate_capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.capacity() {
        | QueueSize::Limitless => return QueueSize::limitless(),
        | QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<Q, E> QueueBase<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.aggregate_len()
  }

  fn capacity(&self) -> QueueSize {
    self.aggregate_capacity()
  }
}

impl<Q, E> QueueWriter<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<Q, E> QueueReader<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up()
  }
}

impl<Q, E> QueueRw<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}
