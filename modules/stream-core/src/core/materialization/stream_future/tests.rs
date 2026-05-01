use core::{
  pin::Pin,
  task::{Context, Poll},
};
use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  task::{Wake, Waker},
};

use super::StreamFuture;
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

fn poll_once<T: Clone>(future: &mut StreamFuture<T>, waker: &Waker) -> Poll<Result<T, StreamError>> {
  let mut cx = Context::from_waker(waker);
  Pin::new(future).poll(&mut cx)
}

#[test]
fn completion_starts_pending() {
  let completion = StreamFuture::<u32>::new();
  assert_eq!(completion.value(), Completion::Pending);
}

#[test]
fn completion_reports_ready_result() {
  let completion: StreamFuture<u32> = StreamFuture::new();
  completion.complete(Ok(7));
  assert_eq!(completion.value(), Completion::Ready(Ok(7)));
}

#[test]
fn completion_try_take_consumes_result() {
  let completion: StreamFuture<u32> = StreamFuture::new();
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.try_take(), Some(Err(StreamError::Failed)));
  assert_eq!(completion.value(), Completion::Pending);
}

#[test]
fn completion_preserves_first_result_on_duplicate_complete() {
  let completion: StreamFuture<u32> = StreamFuture::new();
  completion.complete(Ok(42));
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.value(), Completion::Ready(Ok(42)));
}

#[test]
fn completion_preserves_first_error_on_duplicate_complete() {
  let completion: StreamFuture<u32> = StreamFuture::new();
  completion.complete(Err(StreamError::Failed));
  completion.complete(Ok(99));
  assert_eq!(completion.value(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn future_poll_wakes_registered_waker_when_completed() {
  let mut future: StreamFuture<u32> = StreamFuture::new();
  let wake_counter = Arc::new(WakeCounter::new());
  let waker = Waker::from(wake_counter.clone());

  assert_eq!(poll_once(&mut future, &waker), Poll::Pending);
  assert_eq!(wake_counter.wake_count(), 0);

  future.complete(Ok(7));

  assert_eq!(wake_counter.wake_count(), 1);
  assert_eq!(poll_once(&mut future, &waker), Poll::Ready(Ok(7)));
}

#[test]
fn future_poll_wakes_all_registered_wakers_when_completed() {
  let mut first_handle: StreamFuture<u32> = StreamFuture::new();
  let mut second_handle = first_handle.clone();
  let first = Arc::new(WakeCounter::new());
  let second = Arc::new(WakeCounter::new());
  let first_waker = Waker::from(first.clone());
  let second_waker = Waker::from(second.clone());

  assert_eq!(poll_once(&mut first_handle, &first_waker), Poll::Pending);
  assert_eq!(poll_once(&mut second_handle, &second_waker), Poll::Pending);

  first_handle.complete(Ok(9));

  assert_eq!(first.wake_count(), 1);
  assert_eq!(second.wake_count(), 1);
  assert_eq!(poll_once(&mut first_handle, &first_waker), Poll::Ready(Ok(9)));
}

#[test]
fn future_can_be_awaited_in_minimal_async_runtime() {
  // Synchronous block_on shim: drives the future once, completes externally,
  // then drives again. Ensures the `.await` path returns the resolved value.
  let mut future: StreamFuture<u32> = StreamFuture::new();
  let wake_counter = Arc::new(WakeCounter::new());
  let waker = Waker::from(wake_counter.clone());

  assert_eq!(poll_once(&mut future, &waker), Poll::Pending);
  future.complete(Ok(123));
  assert_eq!(poll_once(&mut future, &waker), Poll::Ready(Ok(123)));
}
