use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  task::{Wake, Waker},
};

use super::StreamCompletion;
use crate::core::{r#impl::StreamError, materialization::Completion};

struct WakeCounter {
  count: AtomicUsize,
}

impl WakeCounter {
  const fn new() -> Self {
    Self { count: AtomicUsize::new(0) }
  }

  fn wake_count(&self) -> usize {
    self.count.load(Ordering::SeqCst)
  }
}

impl Wake for WakeCounter {
  fn wake(self: Arc<Self>) {
    self.count.fetch_add(1, Ordering::SeqCst);
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.count.fetch_add(1, Ordering::SeqCst);
  }
}

#[test]
fn completion_starts_pending() {
  let completion = StreamCompletion::<u32>::new();
  assert_eq!(completion.poll(), Completion::Pending);
}

#[test]
fn completion_reports_ready_result() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Ok(7));
  assert_eq!(completion.poll(), Completion::Ready(Ok(7)));
}

#[test]
fn completion_try_take_consumes_result() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.try_take(), Some(Err(StreamError::Failed)));
  assert_eq!(completion.poll(), Completion::Pending);
}

#[test]
fn completion_preserves_first_result_on_duplicate_complete() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Ok(42));
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.poll(), Completion::Ready(Ok(42)));
}

#[test]
fn completion_preserves_first_error_on_duplicate_complete() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Err(StreamError::Failed));
  completion.complete(Ok(99));
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn completion_wakes_registered_waker_when_completed() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  let wake_counter = Arc::new(WakeCounter::new());
  let waker = Waker::from(wake_counter.clone());

  assert_eq!(completion.poll_with_waker(&waker), Completion::Pending);
  assert_eq!(wake_counter.wake_count(), 0);

  completion.complete(Ok(7));

  assert_eq!(wake_counter.wake_count(), 1);
  assert_eq!(completion.poll(), Completion::Ready(Ok(7)));
}

#[test]
fn completion_wakes_all_registered_wakers_when_completed() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  let first = Arc::new(WakeCounter::new());
  let second = Arc::new(WakeCounter::new());
  let first_waker = Waker::from(first.clone());
  let second_waker = Waker::from(second.clone());

  assert_eq!(completion.poll_with_waker(&first_waker), Completion::Pending);
  assert_eq!(completion.poll_with_waker(&second_waker), Completion::Pending);

  completion.complete(Ok(9));

  assert_eq!(first.wake_count(), 1);
  assert_eq!(second.wake_count(), 1);
  assert_eq!(completion.poll(), Completion::Ready(Ok(9)));
}
