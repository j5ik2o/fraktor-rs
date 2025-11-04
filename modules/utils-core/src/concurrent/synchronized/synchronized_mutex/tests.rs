use alloc::sync::Arc;
use core::{
  ops::{Deref, DerefMut},
  sync::atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;

use crate::{
  concurrent::synchronized::{synchronized_mutex::Synchronized, synchronized_mutex_backend::SynchronizedMutexBackend},
  sync::SharedError,
};

// ?????????????????????
#[derive(Clone, Debug)]
struct MockBackend<T> {
  value:       Arc<T>,
  should_fail: Arc<AtomicBool>,
}

impl<T> MockBackend<T> {
  fn new_success(value: T) -> Self {
    Self { value: Arc::new(value), should_fail: Arc::new(AtomicBool::new(false)) }
  }

  fn new_failing(value: T) -> Self {
    Self { value: Arc::new(value), should_fail: Arc::new(AtomicBool::new(true)) }
  }
}

// Guard????
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
    // ???????????????????
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

  async fn lock(&self) -> Result<Self::Guard<'_>, SharedError> {
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
