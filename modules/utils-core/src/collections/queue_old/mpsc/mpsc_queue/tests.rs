#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::rc::Rc;
use core::{cell::RefCell, fmt};

use crate::collections::{
    QueueError,
    queue_old::mpsc::{MpscBuffer, MpscQueue, mpsc_backend::RingBufferBackend, traits::MpscHandle},
};

struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

impl<T> RcBackendHandle<T> {
  fn new(capacity: Option<usize>) -> Self {
    let buffer = RefCell::new(MpscBuffer::new(capacity));
    let backend = RingBufferBackend::new(buffer);
    Self(Rc::new(backend))
  }
}

impl<T> Clone for RcBackendHandle<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> fmt::Debug for RcBackendHandle<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RcBackendHandle").finish()
  }
}

impl<T> core::ops::Deref for RcBackendHandle<T> {
  type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> crate::sync::Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

impl<T> MpscHandle<T> for RcBackendHandle<T> {
  type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

#[test]
fn buffer_offer_and_poll() {
  let mut buffer: MpscBuffer<u32> = MpscBuffer::new(Some(1));
  assert!(buffer.offer(1).is_ok());
  assert!(matches!(buffer.offer(2), Err(QueueError::Full(2))));
  assert_eq!(buffer.poll().unwrap(), Some(1));
  assert!(buffer.poll().unwrap().is_none());
  buffer.clean_up();
  assert!(matches!(buffer.offer(3), Err(QueueError::Closed(3))));
}

#[test]
fn shared_queue_shared_operations() {
  let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(Some(2)));
  queue.offer(1).unwrap();
  queue.offer(2).unwrap();
  assert!(queue.offer(3).is_err());
  assert_eq!(queue.poll().unwrap(), Some(1));
  assert_eq!(queue.poll().unwrap(), Some(2));
}

#[test]
fn shared_queue_cleanup_marks_closed() {
  let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(None));
  queue.offer(1).unwrap();
  queue.clean_up();
  assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
  assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
}
