use super::SinkQueue;
use crate::core::dsl::SinkQueueWithCancel;

#[test]
fn should_expose_sink_queue_with_cancel_alias_for_pekko_parity() {
  // Given: Pekko publishes `scaladsl.SinkQueueWithCancel[T]` as the cancellable
  // sink queue handle. fraktor-rs expresses the same capability via an alias.
  // When: constructing through the alias.
  let queue = SinkQueueWithCancel::<i32>::new();

  // Then: the instance behaves like a fresh SinkQueue (empty, not cancelled).
  assert!(queue.is_empty());
  assert_eq!(queue.len(), 0);
  assert!(!queue.is_cancelled());
  assert!(queue.pull().is_none());
}

#[test]
fn should_allow_cancel_through_sink_queue_with_cancel_alias() {
  // Pekko's SinkQueueWithCancel adds `cancel()` on top of SinkQueue.
  // The alias must expose the same method without any additional imports.
  let mut queue = SinkQueueWithCancel::<i32>::new();
  assert!(!queue.is_cancelled());
  queue.cancel();
  assert!(queue.is_cancelled());
  assert!(queue.pull().is_none());
}

#[test]
fn should_share_identity_between_sink_queue_and_sink_queue_with_cancel() {
  // `SinkQueueWithCancel<T>` must be a transparent type alias for `SinkQueue<T>`
  // so existing code paths interoperate without conversion.
  let queue: SinkQueue<u8> = SinkQueueWithCancel::<u8>::new();
  let aliased: SinkQueueWithCancel<u8> = SinkQueue::<u8>::new();
  assert!(queue.is_empty());
  assert!(aliased.is_empty());
}

#[test]
fn should_support_clone_via_sink_queue_with_cancel_alias() {
  // Clone propagates shared state identically to SinkQueue.
  let mut queue = SinkQueueWithCancel::<i32>::new();
  let clone: SinkQueueWithCancel<i32> = queue.clone();

  queue.cancel();
  assert!(clone.is_cancelled());
}
