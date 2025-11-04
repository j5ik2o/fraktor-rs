use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};

use async_trait::async_trait;

use crate::concurrent::synchronized::{
  synchronized_rw::SynchronizedRw, synchronized_rw_backend::SynchronizedRwBackend,
};

// ?????????????????????
#[derive(Clone, Debug)]
struct MockBackend<T> {
  value: Arc<T>,
}

// ReadGuard????
struct MockReadGuard<T> {
  value: Arc<T>,
}

impl<T> Deref for MockReadGuard<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

// WriteGuard????
struct MockWriteGuard<T> {
  value: Arc<T>,
}

impl<T> Deref for MockWriteGuard<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

impl<T> DerefMut for MockWriteGuard<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    // ???????????????????
    unsafe { &mut *(Arc::as_ptr(&self.value) as *mut T) }
  }
}

#[async_trait(?Send)]
impl<T> SynchronizedRwBackend<T> for MockBackend<T> {
  type ReadGuard<'a>
    = MockReadGuard<T>
  where
    Self: 'a;
  type WriteGuard<'a>
    = MockWriteGuard<T>
  where
    Self: 'a;

  fn new(value: T) -> Self {
    Self { value: Arc::new(value) }
  }

  async fn read(&self) -> Self::ReadGuard<'_> {
    MockReadGuard { value: Arc::clone(&self.value) }
  }

  async fn write(&self) -> Self::WriteGuard<'_> {
    MockWriteGuard { value: Arc::clone(&self.value) }
  }
}

#[test]
fn new_creates_synchronized_rw() {
  let sync = SynchronizedRw::<MockBackend<i32>, i32>::new(42);
  assert_eq!(*sync.backend().value, 42);
}

#[test]
fn from_backend_creates_synchronized_rw() {
  let backend = MockBackend::new(100);
  let sync = SynchronizedRw::from_backend(backend);
  assert_eq!(*sync.backend().value, 100);
}

#[test]
fn default_creates_with_default_value() {
  let sync = SynchronizedRw::<MockBackend<i32>, i32>::default();
  assert_eq!(*sync.backend().value, 0);
}

#[test]
fn from_value_creates_synchronized_rw() {
  let sync = SynchronizedRw::<MockBackend<i32>, i32>::from(200);
  assert_eq!(*sync.backend().value, 200);
}

#[test]
fn backend_returns_reference() {
  let sync = SynchronizedRw::<MockBackend<i32>, i32>::new(50);
  let backend_ref = sync.backend();
  assert_eq!(*backend_ref.value, 50);
}

#[test]
fn debug_format() {
  let sync = SynchronizedRw::<MockBackend<i32>, i32>::new(75);
  let debug_str = format!("{:?}", sync);
  assert!(debug_str.contains("SynchronizedRw"));
}
