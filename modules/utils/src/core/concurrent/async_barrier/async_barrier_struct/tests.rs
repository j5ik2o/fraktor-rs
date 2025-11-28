use async_trait::async_trait;

use super::AsyncBarrier;
use crate::core::concurrent::async_barrier::async_barrier_backend::AsyncBarrierBackend;

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
fn async_barrier_new() {
  let barrier = AsyncBarrier::<MockBackend>::new(5);
  assert_eq!(barrier.backend().count, 5);
}

#[test]
fn async_barrier_backend() {
  let barrier = AsyncBarrier::<MockBackend>::new(10);
  let backend_ref = barrier.backend();
  assert_eq!(backend_ref.count, 10);
}

#[test]
fn async_barrier_clone() {
  let barrier1 = AsyncBarrier::<MockBackend>::new(3);
  let barrier2 = barrier1.clone();
  assert_eq!(barrier1.backend().count, 3);
  assert_eq!(barrier2.backend().count, 3);
}

#[test]
fn async_barrier_debug() {
  let barrier = AsyncBarrier::<MockBackend>::new(7);
  let debug_str = format!("{:?}", barrier);
  assert!(debug_str.contains("AsyncBarrier"));
}
