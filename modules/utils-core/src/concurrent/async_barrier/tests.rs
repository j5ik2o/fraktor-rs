use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::{async_barrier_backend::AsyncBarrierBackend, async_barrier_struct::AsyncBarrier};

#[derive(Clone, Debug)]
struct MockBackend {
  count: Arc<AtomicUsize>,
}

impl PartialEq for MockBackend {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.count, &other.count) || self.count.load(Ordering::SeqCst) == other.count.load(Ordering::SeqCst)
  }
}

impl Eq for MockBackend {}

#[async_trait(?Send)]
impl AsyncBarrierBackend for MockBackend {
  fn new(count: usize) -> Self {
    Self { count: Arc::new(AtomicUsize::new(count)) }
  }

  async fn wait(&self) {
    self.count.fetch_sub(1, Ordering::SeqCst);
  }
}

#[test]
fn new_creates_barrier_with_count() {
  let barrier = AsyncBarrier::<MockBackend>::new(5);
  assert_eq!(barrier.backend().count.load(Ordering::SeqCst), 5);
}

#[test]
fn backend_returns_reference() {
  let barrier = AsyncBarrier::<MockBackend>::new(10);
  let backend_ref = barrier.backend();
  assert_eq!(backend_ref.count.load(Ordering::SeqCst), 10);
}

#[test]
fn clone_creates_independent_instance() {
  let barrier1 = AsyncBarrier::<MockBackend>::new(3);
  let barrier2 = barrier1.clone();
  assert_eq!(barrier1.backend().count.load(Ordering::SeqCst), 3);
  assert_eq!(barrier2.backend().count.load(Ordering::SeqCst), 3);
}

#[test]
fn debug_format() {
  let barrier = AsyncBarrier::<MockBackend>::new(7);
  let debug_str = format!("{:?}", barrier);
  assert!(debug_str.contains("AsyncBarrier"));
}
