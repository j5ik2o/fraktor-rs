use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};

use async_trait::async_trait;

use crate::core::concurrent::synchronized::{
  synchronized_rw::SynchronizedRw, synchronized_rw_backend::SynchronizedRwBackend,
};

#[derive(Clone, Debug)]
struct MockBackend<T> {
  value: Arc<T>,
}

struct MockReadGuard<T> {
  value: Arc<T>,
}

impl<T> Deref for MockReadGuard<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

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

  async fn read(&mut self) -> Self::ReadGuard<'_> {
    MockReadGuard { value: Arc::clone(&self.value) }
  }

  async fn write(&mut self) -> Self::WriteGuard<'_> {
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

// Helper for async testing
fn block_on<F: core::future::Future>(mut future: F) -> F::Output {
  use core::{
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  };

  fn raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
      raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  let waker = unsafe { Waker::from_raw(raw_waker()) };
  let mut future = unsafe { Pin::new_unchecked(&mut future) };
  let mut context = Context::from_waker(&waker);

  loop {
    match future.as_mut().poll(&mut context) {
      | Poll::Ready(output) => return output,
      | Poll::Pending => continue,
    }
  }
}

#[test]
fn read_executes_closure() {
  let mut sync = SynchronizedRw::<MockBackend<i32>, i32>::new(100);
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 100);
}

#[test]
fn write_executes_closure() {
  let mut sync = SynchronizedRw::<MockBackend<i32>, i32>::new(50);
  block_on(sync.write(|guard| {
    **guard = 300;
  }));
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 300);
}

#[test]
fn read_guard_returns_handle() {
  let mut sync = SynchronizedRw::<MockBackend<i32>, i32>::new(15);
  let guard = block_on(sync.read_guard());
  assert_eq!(**guard, 15);
}

#[test]
fn write_guard_returns_handle() {
  let mut sync = SynchronizedRw::<MockBackend<i32>, i32>::new(20);
  let mut guard = block_on(sync.write_guard());
  **guard = 60;
  drop(guard);
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 60);
}
