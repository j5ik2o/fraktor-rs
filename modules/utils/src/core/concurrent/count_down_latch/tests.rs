use async_trait::async_trait;

use super::{count_down_latch_backend::CountDownLatchBackend, count_down_latch_struct::CountDownLatch};

#[derive(Clone, Debug, PartialEq, Eq)]
struct MockBackend {
  count: usize,
}

#[async_trait(?Send)]
impl CountDownLatchBackend for MockBackend {
  fn new(count: usize) -> Self {
    Self { count }
  }

  async fn count_down(&mut self) {
    self.count = self.count.saturating_sub(1);
  }

  async fn wait(&mut self) {
    // Note: In real usage, count_down would be called from other threads
    #[allow(clippy::while_immutable_condition)]
    while self.count > 0 {
      core::hint::spin_loop();
    }
  }
}

#[test]
fn new_creates_latch_with_count() {
  let latch = CountDownLatch::<MockBackend>::new(5);
  assert_eq!(latch.backend().count, 5);
}

#[test]
fn default_creates_latch_with_zero_count() {
  let latch = CountDownLatch::<MockBackend>::default();
  assert_eq!(latch.backend().count, 0);
}

#[test]
fn backend_returns_reference() {
  let latch = CountDownLatch::<MockBackend>::new(10);
  let backend_ref = latch.backend();
  assert_eq!(backend_ref.count, 10);
}

#[test]
fn clone_creates_independent_instance() {
  let latch1 = CountDownLatch::<MockBackend>::new(3);
  let latch2 = latch1.clone();
  assert_eq!(latch1.backend().count, 3);
  assert_eq!(latch2.backend().count, 3);
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
