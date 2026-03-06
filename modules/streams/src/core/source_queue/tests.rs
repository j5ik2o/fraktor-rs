use crate::core::{QueueOfferResult, SourceQueue, StreamError};

#[test]
fn source_queue_should_enqueue_and_poll_in_fifo_order() {
  let queue = SourceQueue::new();
  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  assert_eq!(queue.offer(2_u32), QueueOfferResult::Enqueued);

  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert_eq!(queue.poll().expect("poll"), Some(2_u32));
  assert_eq!(queue.poll().expect("poll"), None);
}

#[test]
fn source_queue_should_reject_offer_after_complete() {
  let queue = SourceQueue::<u32>::new();
  queue.complete();

  assert_eq!(queue.offer(10_u32), QueueOfferResult::QueueClosed);
  assert!(queue.is_closed());
}

#[test]
fn source_queue_should_return_failure_after_fail() {
  let queue = SourceQueue::<u32>::new();
  queue.fail(StreamError::Failed);

  assert_eq!(queue.offer(10_u32), QueueOfferResult::Failure(StreamError::Failed));
  assert!(matches!(queue.poll(), Err(StreamError::Failed)));
}

#[test]
fn source_queue_should_report_drained_after_complete_and_poll() {
  let queue = SourceQueue::<u32>::new();
  assert_eq!(queue.offer(1_u32), QueueOfferResult::Enqueued);
  queue.complete();

  assert!(!queue.is_drained());
  assert_eq!(queue.poll().expect("poll"), Some(1_u32));
  assert!(queue.is_drained());
}
