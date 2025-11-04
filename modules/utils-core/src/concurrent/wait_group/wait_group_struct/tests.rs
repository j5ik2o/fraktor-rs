use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::WaitGroup;
use crate::concurrent::wait_group::wait_group_backend::WaitGroupBackend;

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
impl WaitGroupBackend for MockBackend {
  fn new() -> Self {
    Self { count: Arc::new(AtomicUsize::new(0)) }
  }

  fn with_count(count: usize) -> Self {
    Self { count: Arc::new(AtomicUsize::new(count)) }
  }

  fn add(&self, n: usize) {
    self.count.fetch_add(n, Ordering::SeqCst);
  }

  fn done(&self) {
    self.count.fetch_sub(1, Ordering::SeqCst);
  }

  async fn wait(&self) {
    while self.count.load(Ordering::SeqCst) > 0 {
      core::hint::spin_loop();
    }
  }
}

#[test]
fn wait_group_new() {
  let wg = WaitGroup::<MockBackend>::new();
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 0);
}

#[test]
fn wait_group_with_count() {
  let wg = WaitGroup::<MockBackend>::with_count(5);
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 5);
}

#[test]
fn wait_group_default() {
  let wg = WaitGroup::<MockBackend>::default();
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 0);
}

#[test]
fn wait_group_add() {
  let wg = WaitGroup::<MockBackend>::new();
  wg.add(3);
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 3);
  wg.add(2);
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 5);
}

#[test]
fn wait_group_done() {
  let wg = WaitGroup::<MockBackend>::with_count(5);
  wg.done();
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 4);
  wg.done();
  assert_eq!(wg.backend().count.load(Ordering::SeqCst), 3);
}

#[test]
fn wait_group_backend() {
  let wg = WaitGroup::<MockBackend>::with_count(10);
  let backend_ref = wg.backend();
  assert_eq!(backend_ref.count.load(Ordering::SeqCst), 10);
}

#[test]
fn wait_group_clone() {
  let wg1 = WaitGroup::<MockBackend>::with_count(3);
  let wg2 = wg1.clone();
  assert_eq!(wg1.backend().count.load(Ordering::SeqCst), 3);
  assert_eq!(wg2.backend().count.load(Ordering::SeqCst), 3);
}

#[test]
fn wait_group_debug() {
  let wg = WaitGroup::<MockBackend>::with_count(7);
  let debug_str = format!("{:?}", wg);
  assert!(debug_str.contains("WaitGroup"));
}
