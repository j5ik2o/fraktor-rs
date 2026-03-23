use crate::core::{ActorSourceRef, BoundedSourceQueue, OverflowStrategy, QueueOfferResult, StreamError};

// --- tell ---

#[test]
fn actor_source_ref_tell_enqueues_value() {
  // Given: an ActorSourceRef backed by a queue of capacity 4
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);

  // When: sending a value via tell
  let result = source_ref.tell(42_u32);

  // Then: the value is enqueued
  assert_eq!(result, QueueOfferResult::Enqueued);
}

#[test]
fn actor_source_ref_tell_respects_overflow_strategy() {
  // Given: an ActorSourceRef backed by a queue of capacity 1 with Fail strategy
  let queue = BoundedSourceQueue::new(1, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);

  // When: sending more values than the buffer can hold
  let first = source_ref.tell(1_u32);
  let second = source_ref.tell(2_u32);

  // Then: first succeeds, second fails with BufferOverflow
  assert_eq!(first, QueueOfferResult::Enqueued);
  assert_eq!(second, QueueOfferResult::Failure(StreamError::BufferOverflow));
}

#[test]
fn actor_source_ref_tell_returns_queue_closed_after_complete() {
  // Given: an ActorSourceRef that has been completed
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);
  source_ref.complete();

  // When: attempting to tell after completion
  let result = source_ref.tell(1_u32);

  // Then: QueueClosed is returned
  assert_eq!(result, QueueOfferResult::QueueClosed);
}

// --- complete ---

#[test]
fn actor_source_ref_complete_closes_queue() {
  // Given: an open ActorSourceRef
  let queue = BoundedSourceQueue::<u32>::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);
  assert!(!source_ref.is_closed());

  // When: completing
  source_ref.complete();

  // Then: the queue is closed
  assert!(source_ref.is_closed());
}

// --- fail ---

#[test]
fn actor_source_ref_fail_closes_queue_with_error() {
  // Given: an open ActorSourceRef
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);

  // When: failing with an error
  source_ref.fail(StreamError::Failed);

  // Then: the queue is closed and subsequent tells report the failure
  assert!(source_ref.is_closed());
  assert_eq!(source_ref.tell(1_u32), QueueOfferResult::Failure(StreamError::Failed));
}

// --- is_closed ---

#[test]
fn actor_source_ref_is_closed_returns_false_when_open() {
  // Given: a freshly created ActorSourceRef
  let queue = BoundedSourceQueue::<u32>::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);

  // Then: is_closed returns false
  assert!(!source_ref.is_closed());
}

// --- Clone ---

#[test]
fn actor_source_ref_clone_shares_queue() {
  // Given: an ActorSourceRef and its clone
  let queue = BoundedSourceQueue::new(4, OverflowStrategy::Fail);
  let source_ref = ActorSourceRef::new(queue);
  let cloned = source_ref.clone();

  // When: telling via the original
  let _ = source_ref.tell(1_u32);

  // When: completing via the clone
  cloned.complete();

  // Then: both see the closed state
  assert!(source_ref.is_closed());
  assert!(cloned.is_closed());
}
