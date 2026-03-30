use core::{
  future::Future,
  pin::pin,
  task::{Context, Poll, Waker},
};
use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  task::Wake,
};

use super::SourceQueueWithComplete;
use crate::core::{
  OverflowStrategy, QueueOfferResult, StreamError,
  materialization::{Completion, StreamDone},
};

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
fn source_queue_with_complete_should_enqueue_and_complete_after_drain() {
  let mut queue = SourceQueueWithComplete::new(2, OverflowStrategy::DropTail, 1);
  let completion = queue.watch_completion();

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);
  assert_eq!(poll_ready(queue.offer(2_u32)), QueueOfferResult::Enqueued);
  assert_eq!(completion.poll(), Completion::Pending);

  queue.complete();
  assert_eq!(completion.poll(), Completion::Pending);

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(completion.poll(), Completion::Pending);
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn source_queue_with_complete_should_wait_for_space_on_backpressure() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);
  let mut waiting_offer = pin!(queue.offer(2_u32));
  let (waker, wake_counter) = tracking_waker();
  let mut context = Context::from_waker(&waker);
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(wake_counter.wake_count(), 0);
  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(wake_counter.wake_count(), 1);
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_complete_should_fail_offer_when_pending_offer_limit_is_exceeded() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);

  let mut first_pending_offer = pin!(queue.offer(2_u32));
  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Pending);

  assert_eq!(poll_ready(queue.offer(3_u32)), QueueOfferResult::Failure(StreamError::WouldBlock));

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_complete_should_allow_zero_capacity() {
  let mut queue = SourceQueueWithComplete::new(0, OverflowStrategy::Backpressure, 1);
  let mut waiting_offer = pin!(queue.offer(10_u32));
  let (waker, wake_counter) = tracking_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(wake_counter.wake_count(), 0);
  assert_eq!(queue.poll().expect("poll"), Some(10_u32));
  assert_eq!(wake_counter.wake_count(), 1);
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_complete_should_reject_offer_after_complete() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::DropTail, 1);
  let completion = queue.watch_completion();

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);

  queue.complete();

  assert_eq!(poll_ready(queue.offer(2_u32)), QueueOfferResult::QueueClosed);
  assert_eq!(completion.poll(), Completion::Pending);
  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn source_queue_with_complete_should_fail_offer_and_completion_after_fail() {
  let mut queue = SourceQueueWithComplete::<u32>::new(1, OverflowStrategy::DropTail, 1);
  let completion = queue.watch_completion();

  queue.fail(StreamError::Failed);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Failure(StreamError::Failed));
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn source_queue_with_complete_should_fail_pending_offer_after_fail() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);

  let mut pending_offer = pin!(queue.offer(2_u32));
  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Pending);

  queue.fail(StreamError::Failed);

  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Failure(StreamError::Failed)));
  assert_eq!(poll_ready(queue.offer(3_u32)), QueueOfferResult::Failure(StreamError::Failed));
}

#[test]
fn source_queue_with_complete_should_allow_configured_number_of_pending_offers() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 2);
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);

  let mut first_pending_offer = pin!(queue.offer(2_u32));
  let mut second_pending_offer = pin!(queue.offer(3_u32));

  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(second_pending_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(poll_ready(queue.offer(4_u32)), QueueOfferResult::Failure(StreamError::WouldBlock));

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(first_pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(second_pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(3_u32));
}

#[test]
fn source_queue_with_complete_close_for_cancel_should_resolve_pending_offer_and_completion() {
  let mut queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
  let completion = queue.watch_completion();
  let (waker, wake_counter) = tracking_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);

  let mut pending_offer = pin!(queue.offer(2_u32));
  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(wake_counter.wake_count(), 0);

  queue.close_for_cancel();

  assert_eq!(wake_counter.wake_count(), 1);
  assert_eq!(pending_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::QueueClosed));
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
  assert!(queue.is_closed());
  assert!(queue.is_empty());
}

fn poll_ready<F>(future: F) -> F::Output
where
  F: Future, {
  let mut future = pin!(future);
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  match future.as_mut().poll(&mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future should be ready"),
  }
}

fn noop_waker() -> Waker {
  Waker::noop().clone()
}

fn tracking_waker() -> (Waker, Arc<WakeCounter>) {
  let wake_counter = Arc::new(WakeCounter::new());
  (Waker::from(wake_counter.clone()), wake_counter)
}
