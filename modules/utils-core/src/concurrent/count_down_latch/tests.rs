use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::{count_down_latch_backend::CountDownLatchBackend, count_down_latch_struct::CountDownLatch};

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
impl CountDownLatchBackend for MockBackend {
  fn new(count: usize) -> Self {
    Self { count: Arc::new(AtomicUsize::new(count)) }
  }

  async fn count_down(&self) {
    self.count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn wait(&self) {
    while self.count.load(Ordering::SeqCst) > 0 {
      core::hint::spin_loop();
    }
  }
}

#[test]
fn new_creates_latch_with_count() {
  let latch = CountDownLatch::<MockBackend>::new(5);
  assert_eq!(latch.backend().count.load(Ordering::SeqCst), 5);
}

#[test]
fn default_creates_latch_with_zero_count() {
  let latch = CountDownLatch::<MockBackend>::default();
  assert_eq!(latch.backend().count.load(Ordering::SeqCst), 0);
}

#[test]
fn backend_returns_reference() {
  let latch = CountDownLatch::<MockBackend>::new(10);
  let backend_ref = latch.backend();
  assert_eq!(backend_ref.count.load(Ordering::SeqCst), 10);
}

#[test]
fn clone_creates_independent_instance() {
  let latch1 = CountDownLatch::<MockBackend>::new(3);
  let latch2 = latch1.clone();
  assert_eq!(latch1.backend().count.load(Ordering::SeqCst), 3);
  assert_eq!(latch2.backend().count.load(Ordering::SeqCst), 3);
}

#[test]
fn debug_format() {
  let latch = CountDownLatch::<MockBackend>::new(7);
  let debug_str = format!("{:?}", latch);
  assert!(debug_str.contains("CountDownLatch"));
}

#[test]
fn partial_eq_works() {
  let latch1 = CountDownLatch::<MockBackend>::new(5);
  let latch2 = latch1.clone();
  assert_eq!(latch1, latch2);
}
