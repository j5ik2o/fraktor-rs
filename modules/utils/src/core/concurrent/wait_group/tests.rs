use async_trait::async_trait;

use super::{wait_group_backend::WaitGroupBackend, wait_group_struct::WaitGroup};

#[derive(Clone, Debug, PartialEq, Eq)]
struct MockBackend {
  count: usize,
}

#[async_trait(?Send)]
impl WaitGroupBackend for MockBackend {
  fn new() -> Self {
    Self { count: 0 }
  }

  fn with_count(count: usize) -> Self {
    Self { count }
  }

  fn add(&mut self, n: usize) {
    self.count += n;
  }

  fn done(&mut self) {
    self.count = self.count.saturating_sub(1);
  }

  async fn wait(&mut self) {
    // Note: In real usage, done would be called from other threads
    #[allow(clippy::while_immutable_condition)]
    while self.count > 0 {
      core::hint::spin_loop();
    }
  }
}

#[test]
fn new_creates_wait_group_with_zero_count() {
  let wg = WaitGroup::<MockBackend>::new();
  assert_eq!(wg.backend().count, 0);
}

#[test]
fn with_count_creates_wait_group_with_specified_count() {
  let wg = WaitGroup::<MockBackend>::with_count(5);
  assert_eq!(wg.backend().count, 5);
}

#[test]
fn default_creates_wait_group_with_zero_count() {
  let wg = WaitGroup::<MockBackend>::default();
  assert_eq!(wg.backend().count, 0);
}

#[test]
fn add_increments_count() {
  let mut wg = WaitGroup::<MockBackend>::new();
  wg.add(3);
  assert_eq!(wg.backend().count, 3);
  wg.add(2);
  assert_eq!(wg.backend().count, 5);
}

#[test]
fn done_decrements_count() {
  let mut wg = WaitGroup::<MockBackend>::with_count(5);
  wg.done();
  assert_eq!(wg.backend().count, 4);
  wg.done();
  assert_eq!(wg.backend().count, 3);
}

#[test]
fn backend_returns_reference() {
  let wg = WaitGroup::<MockBackend>::with_count(10);
  let backend_ref = wg.backend();
  assert_eq!(backend_ref.count, 10);
}

#[test]
fn clone_creates_independent_instance() {
  let wg1 = WaitGroup::<MockBackend>::with_count(3);
  let wg2 = wg1.clone();
  assert_eq!(wg1.backend().count, 3);
  assert_eq!(wg2.backend().count, 3);
}

#[test]
fn debug_format() {
  let wg = WaitGroup::<MockBackend>::with_count(7);
  let debug_str = format!("{:?}", wg);
  assert!(debug_str.contains("WaitGroup"));
}
