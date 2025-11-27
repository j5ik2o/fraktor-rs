use alloc::sync::Arc;
use core::{
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;

use crate::core::{
  concurrent::synchronized::{synchronized_mutex::Synchronized, synchronized_mutex_backend::SynchronizedMutexBackend},
  sync::SharedError,
};

#[derive(Clone, Debug)]
struct MockBackend<T> {
  value:       Arc<T>,
  should_fail: Arc<AtomicBool>,
}

impl<T> MockBackend<T> {
  fn new_success(value: T) -> Self {
    Self { value: Arc::new(value), should_fail: Arc::new(AtomicBool::new(false)) }
  }

  #[allow(dead_code)]
  fn new_failing(value: T) -> Self {
    Self { value: Arc::new(value), should_fail: Arc::new(AtomicBool::new(true)) }
  }
}

struct MockGuard<T> {
  value: Arc<T>,
}

impl<T> Deref for MockGuard<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

impl<T> DerefMut for MockGuard<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *(Arc::as_ptr(&self.value) as *mut T) }
  }
}

#[async_trait(?Send)]
impl<T> SynchronizedMutexBackend<T> for MockBackend<T> {
  type Guard<'a>
    = MockGuard<T>
  where
    Self: 'a;

  fn new(value: T) -> Self {
    Self::new_success(value)
  }

  async fn lock(&mut self) -> Result<Self::Guard<'_>, SharedError> {
    if self.should_fail.load(Ordering::SeqCst) {
      Err(SharedError::InterruptContext)
    } else {
      Ok(MockGuard { value: Arc::clone(&self.value) })
    }
  }
}

#[test]
fn new_creates_synchronized() {
  let sync = Synchronized::<MockBackend<i32>, i32>::new(42);
  assert_eq!(*sync.backend().value, 42);
}

#[test]
fn from_backend_creates_synchronized() {
  let backend = MockBackend::new(100);
  let sync = Synchronized::from_backend(backend);
  assert_eq!(*sync.backend().value, 100);
}

#[test]
fn default_creates_with_default_value() {
  let sync = Synchronized::<MockBackend<i32>, i32>::default();
  assert_eq!(*sync.backend().value, 0);
}

#[test]
fn from_value_creates_synchronized() {
  let sync = Synchronized::<MockBackend<i32>, i32>::from(200);
  assert_eq!(*sync.backend().value, 200);
}

#[test]
fn backend_returns_reference() {
  let sync = Synchronized::<MockBackend<i32>, i32>::new(50);
  let backend_ref = sync.backend();
  assert_eq!(*backend_ref.value, 50);
}

#[test]
fn debug_format() {
  let sync = Synchronized::<MockBackend<i32>, i32>::new(75);
  let debug_str = format!("{:?}", sync);
  assert!(debug_str.contains("Synchronized"));
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
fn read_executes_closure_with_guard() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::new(100);
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 100);
}

#[test]
fn write_executes_closure_with_mutable_guard() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::new(50);
  block_on(sync.write(|guard| {
    **guard = 200;
  }));
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 200);
}

#[test]
fn write_modifies_and_read_verifies() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::new(10);
  block_on(sync.write(|guard| {
    **guard *= 2;
  }));
  let result = block_on(sync.read(|guard| **guard));
  assert_eq!(result, 20);
}

#[test]
#[should_panic(expected = "Synchronized::read requires blocking to be allowed")]
fn read_panics_on_lock_failure() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::from_backend(MockBackend::new_failing(100));
  block_on(sync.read(|_| {}));
}

#[test]
#[should_panic(expected = "Synchronized::write requires blocking to be allowed")]
fn write_panics_on_lock_failure() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::from_backend(MockBackend::new_failing(100));
  block_on(sync.write(|_| {}));
}

#[test]
fn lock_returns_guard_handle() {
  let mut sync = Synchronized::<MockBackend<i32>, i32>::new(30);
  let guard = block_on(sync.lock());
  assert_eq!(**guard, 30);
}
