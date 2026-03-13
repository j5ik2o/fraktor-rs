use crate::core::{BoundedSourceQueue, OverflowStrategy, QueueOfferResult, StreamError};

#[test]
fn bounded_source_queue_should_enqueue_until_capacity() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.len(), 2);
}

#[test]
#[should_panic(expected = "capacity must be greater than zero")]
fn bounded_source_queue_should_panic_when_capacity_is_zero() {
  let _ = BoundedSourceQueue::<u32>::new(0, OverflowStrategy::DropTail);
}

#[test]
fn bounded_source_queue_should_drop_head_when_configured() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropHead);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(3_u32), QueueOfferResult::Enqueued);

  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), Some(3_u32));
}

#[test]
fn bounded_source_queue_should_drop_tail_when_configured() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(3_u32), QueueOfferResult::Enqueued);

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(queue.poll().expect("poll"), Some(3_u32));
}

#[test]
fn bounded_source_queue_should_fail_offer_when_backpressure_and_full() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::Backpressure);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(3_u32), QueueOfferResult::Failure(StreamError::WouldBlock));

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn bounded_source_queue_should_drop_buffer_and_keep_latest() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropBuffer);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(3_u32), QueueOfferResult::Enqueued);

  assert_eq!(queue.poll().expect("poll"), Some(3_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn bounded_source_queue_should_fail_when_configured() {
  let queue = BoundedSourceQueue::new(1, OverflowStrategy::Fail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Failure(StreamError::BufferOverflow));
  assert!(matches!(queue.poll(), Err(StreamError::BufferOverflow)));
}

#[test]
fn bounded_source_queue_should_report_queue_closed_after_complete() {
  let queue = BoundedSourceQueue::new(1, OverflowStrategy::DropTail);
  queue.complete();

  assert_eq!(queue.offer(1_u32), QueueOfferResult::QueueClosed);
  assert!(queue.is_closed());
}

#[test]
fn bounded_source_queue_should_drain_buffer_after_complete() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);

  queue.complete();

  assert_eq!(queue.offer(3_u32), QueueOfferResult::QueueClosed);
  assert!(!queue.is_drained());
  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert!(!queue.is_drained());
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert!(queue.is_drained());
}

#[test]
fn bounded_source_queue_should_fail_offer_and_poll_after_fail() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);

  queue.fail(StreamError::Failed);

  assert_eq!(queue.offer(2_u32), QueueOfferResult::Failure(StreamError::Failed));
  assert!(queue.is_closed());
  assert!(matches!(queue.poll(), Err(StreamError::Failed)));
}

#[test]
#[should_panic(expected = "bounded source queue already terminated: complete")]
fn bounded_source_queue_should_panic_when_completed_twice() {
  let queue = BoundedSourceQueue::<u32>::new(1, OverflowStrategy::DropTail);

  queue.complete();
  queue.complete();
}

#[test]
#[should_panic(expected = "bounded source queue already terminated: fail")]
fn bounded_source_queue_should_panic_when_failed_twice() {
  let queue = BoundedSourceQueue::<u32>::new(1, OverflowStrategy::DropTail);

  queue.fail(StreamError::Failed);
  queue.fail(StreamError::Failed);
}

#[test]
fn bounded_source_queue_fail_if_open_should_ignore_already_completed_queue() {
  let queue = BoundedSourceQueue::<u32>::new(1, OverflowStrategy::DropTail);

  queue.complete();
  assert!(!queue.fail_if_open(StreamError::Failed));
  assert!(queue.is_closed());
}

#[test]
fn bounded_source_queue_close_for_cancel_should_discard_buffered_values() {
  let queue = BoundedSourceQueue::new(2, OverflowStrategy::DropTail);

  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);

  queue.close_for_cancel();

  assert!(queue.is_closed());
  assert!(queue.is_empty());
  assert!(queue.is_drained());
  assert_eq!(queue.offer(3_u32), QueueOfferResult::QueueClosed);
}
