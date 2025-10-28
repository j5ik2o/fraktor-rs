#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::rc::Rc;
use core::cell::RefCell;

use super::{super::RingBuffer, RingQueue};
use crate::collections::queue::{
  ring::{ring_handle::RingHandle, ring_storage_backend::RingStorageBackend},
  traits::QueueHandle,
};

struct RcStorageHandle<E>(Rc<RefCell<RingBuffer<E>>>);

impl<E> Clone for RcStorageHandle<E> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<E> core::ops::Deref for RcStorageHandle<E> {
  type Target = RefCell<RingBuffer<E>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<E> QueueHandle<E> for RcStorageHandle<E> {
  type Storage = RefCell<RingBuffer<E>>;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

impl<E> crate::sync::Shared<RefCell<RingBuffer<E>>> for RcStorageHandle<E> {}

struct RcBackendHandle<E>(Rc<RingStorageBackend<RcStorageHandle<E>>>);

impl<E> Clone for RcBackendHandle<E> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<E> core::ops::Deref for RcBackendHandle<E> {
  type Target = RingStorageBackend<RcStorageHandle<E>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<E> crate::sync::Shared<RingStorageBackend<RcStorageHandle<E>>> for RcBackendHandle<E> {}

impl<E> RingHandle<E> for RcBackendHandle<E> {
  type Backend = RingStorageBackend<RcStorageHandle<E>>;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

#[test]
fn shared_ring_queue_offer_poll() {
  let storage = RcStorageHandle(Rc::new(RefCell::new(RingBuffer::new(2))));
  let backend = RcBackendHandle(Rc::new(RingStorageBackend::new(storage)));
  let queue = RingQueue::new(backend);

  queue.offer(1).unwrap();
  queue.offer(2).unwrap();
  assert_eq!(queue.poll().unwrap(), Some(1));
  assert_eq!(queue.poll().unwrap(), Some(2));
  assert_eq!(queue.poll().unwrap(), None);
}
