#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::rc::Rc;
use core::cell::RefCell;

use super::*;
use crate::{
  collections::stack::{
    buffer::StackBuffer,
    traits::{StackHandle, StackStorage, StackStorageBackend},
  },
  sync::Shared,
};

struct RcStorageHandle<T>(Rc<RefCell<StackBuffer<T>>>);

impl<T> Clone for RcStorageHandle<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> core::ops::Deref for RcStorageHandle<T> {
  type Target = RefCell<StackBuffer<T>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> StackStorage<T> for RcStorageHandle<T> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    f(&self.borrow())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    f(&mut self.borrow_mut())
  }
}

impl<T> Shared<RefCell<StackBuffer<T>>> for RcStorageHandle<T> {}

struct RcBackendHandle<T>(Rc<StackStorageBackend<RcStorageHandle<T>>>);

impl<T> Clone for RcBackendHandle<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> core::ops::Deref for RcBackendHandle<T> {
  type Target = StackStorageBackend<RcStorageHandle<T>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Shared<StackStorageBackend<RcStorageHandle<T>>> for RcBackendHandle<T> {}

impl<T> StackHandle<T> for RcBackendHandle<T> {
  type Backend = StackStorageBackend<RcStorageHandle<T>>;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

#[test]
fn stack_push_pop_via_handle() {
  let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
  let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
  let stack = Stack::new(backend.clone());

  stack.set_capacity(Some(2));
  stack.push(1).unwrap();
  stack.push(2).unwrap();
  assert!(stack.push(3).is_err());
  assert_eq!(stack.pop(), Some(2));
  assert_eq!(backend.backend().len().to_usize(), 1);
}

#[test]
fn stack_peek_via_handle() {
  let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
  let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
  let stack = Stack::new(backend);

  stack.push(7).unwrap();
  assert_eq!(stack.peek(), Some(7));
  stack.pop();
  assert_eq!(stack.peek(), None);
}

#[test]
fn stack_clear_via_handle() {
  let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
  let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
  let stack = Stack::new(backend);

  stack.push(1).unwrap();
  stack.clear();
  assert!(stack.is_empty());
}
