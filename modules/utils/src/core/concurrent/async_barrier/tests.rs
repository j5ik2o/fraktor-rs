use async_trait::async_trait;

use super::{async_barrier_backend::AsyncBarrierBackend, async_barrier_struct::AsyncBarrier};

#[derive(Clone, Debug, PartialEq, Eq)]
struct MockBackend {
  count: usize,
}

#[async_trait(?Send)]
impl AsyncBarrierBackend for MockBackend {
  fn new(count: usize) -> Self {
    Self { count }
  }

  async fn wait(&mut self) {
    self.count = self.count.saturating_sub(1);
  }
}

#[test]
fn new_creates_barrier_with_count() {
  let barrier = AsyncBarrier::<MockBackend>::new(5);
  assert_eq!(barrier.backend().count, 5);
}

#[test]
fn backend_returns_reference() {
  let barrier = AsyncBarrier::<MockBackend>::new(10);
  let backend_ref = barrier.backend();
  assert_eq!(backend_ref.count, 10);
}

#[test]
fn clone_creates_independent_instance() {
  let barrier1 = AsyncBarrier::<MockBackend>::new(3);
  let barrier2 = barrier1.clone();
  assert_eq!(barrier1.backend().count, 3);
  assert_eq!(barrier2.backend().count, 3);
}

#[test]
fn debug_format() {
  let barrier = AsyncBarrier::<MockBackend>::new(7);
  let debug_str = format!("{:?}", barrier);
  assert!(debug_str.contains("AsyncBarrier"));
}
