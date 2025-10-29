#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

use super::{PriorityMessage, PriorityQueue};
use crate::{
  collections::{
    QueueError, QueueSize,
    queue_old::{
      mpsc::{MpscBuffer, MpscHandle, MpscQueue, RingBufferBackend},
      traits::{QueueBase, QueueReader, QueueRw, QueueWriter},
    },
  },
  sync::Shared,
};

#[derive(Debug, Clone)]
struct TestQueue(MpscQueue<RcHandle<u32>, u32>);

#[derive(Debug)]
struct RcHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

impl<T> RcHandle<T> {
  fn new(capacity: Option<usize>) -> Self {
    let buffer = RefCell::new(MpscBuffer::new(capacity));
    let backend = RingBufferBackend::new(buffer);
    Self(Rc::new(backend))
  }
}

impl<T> core::ops::Deref for RcHandle<T> {
  type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Clone for RcHandle<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcHandle<T> {}

impl<T> MpscHandle<T> for RcHandle<T> {
  type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

impl PriorityMessage for u32 {
  fn get_priority(&self) -> Option<i8> {
    Some((*self % 8) as i8)
  }
}

impl QueueRw<u32> for TestQueue {
  fn offer(&self, element: u32) -> Result<(), QueueError<u32>> {
    self.0.offer(element)
  }

  fn poll(&self) -> Result<Option<u32>, QueueError<u32>> {
    self.0.poll()
  }

  fn clean_up(&self) {
    self.0.clean_up();
  }
}

impl QueueBase<u32> for TestQueue {
  fn len(&self) -> QueueSize {
    self.0.len()
  }

  fn capacity(&self) -> QueueSize {
    self.0.capacity()
  }
}

impl QueueWriter<u32> for TestQueue {
  fn offer_mut(&mut self, element: u32) -> Result<(), QueueError<u32>> {
    self.0.offer_mut(element)
  }
}

impl QueueReader<u32> for TestQueue {
  fn poll_mut(&mut self) -> Result<Option<u32>, QueueError<u32>> {
    self.0.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.0.clean_up_mut();
  }
}

impl TestQueue {
  fn bounded(cap: usize) -> Self {
    Self(MpscQueue::new(RcHandle::new(Some(cap))))
  }

  fn unbounded() -> Self {
    Self(MpscQueue::new(RcHandle::new(None)))
  }
}

fn sample_levels() -> Vec<TestQueue> {
  (0..super::PRIORITY_LEVELS).map(|_| TestQueue::bounded(4)).collect()
}

#[test]
fn shared_priority_queue_orders_by_priority() {
  let queue = PriorityQueue::new(sample_levels());
  queue.offer(1).unwrap();
  queue.offer(15).unwrap();
  queue.offer(7).unwrap();

  assert_eq!(queue.poll().unwrap(), Some(15));
  assert_eq!(queue.poll().unwrap(), Some(7));
  assert_eq!(queue.poll().unwrap(), Some(1));
  assert_eq!(queue.poll().unwrap(), None);
}

#[test]
fn shared_priority_queue_len_and_capacity() {
  let queue = PriorityQueue::new(sample_levels());
  let expected = QueueSize::limited(super::PRIORITY_LEVELS * 4);
  assert_eq!(queue.capacity(), expected);
  queue.offer(3).unwrap();
  assert_eq!(queue.len(), QueueSize::limited(1));
  queue.clean_up();
  assert_eq!(queue.len(), QueueSize::limited(0));
}

#[test]
fn shared_priority_queue_unbounded_capacity() {
  let levels = (0..super::PRIORITY_LEVELS).map(|_| TestQueue::unbounded()).collect();
  let queue = PriorityQueue::new(levels);
  assert!(queue.capacity().is_limitless());
}
