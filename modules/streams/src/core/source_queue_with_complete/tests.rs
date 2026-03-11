use core::{
  future::Future,
  pin::pin,
  task::{Context, Poll, Waker},
};

use crate::core::{Completion, OverflowStrategy, QueueOfferResult, SourceQueueWithComplete, StreamDone, StreamError};

#[test]
fn source_queue_with_complete_should_enqueue_and_complete_after_drain() {
  let queue = SourceQueueWithComplete::new(2, OverflowStrategy::DropTail, 1);
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
  let queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Enqueued);
  let mut waiting_offer = pin!(queue.offer(2_u32));
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_complete_should_fail_offer_when_pending_offer_limit_is_exceeded() {
  let queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
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
  let queue = SourceQueueWithComplete::new(0, OverflowStrategy::Backpressure, 1);
  let mut waiting_offer = pin!(queue.offer(10_u32));
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Pending);
  assert_eq!(queue.poll().expect("poll"), Some(10_u32));
  assert_eq!(waiting_offer.as_mut().poll(&mut context), Poll::Ready(QueueOfferResult::Enqueued));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_with_complete_should_reject_offer_after_complete() {
  let queue = SourceQueueWithComplete::new(1, OverflowStrategy::DropTail, 1);
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
  let queue = SourceQueueWithComplete::<u32>::new(1, OverflowStrategy::DropTail, 1);
  let completion = queue.watch_completion();

  queue.fail(StreamError::Failed);

  assert_eq!(poll_ready(queue.offer(1_u32)), QueueOfferResult::Failure(StreamError::Failed));
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn source_queue_with_complete_should_fail_pending_offer_after_fail() {
  let queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 1);
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
  let queue = SourceQueueWithComplete::new(1, OverflowStrategy::Backpressure, 2);
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
