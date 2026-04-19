use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{
    BoundedStablePriorityMessageQueueState, BoundedStablePriorityMessageQueueStateShared, EnqueueOutcome,
    MailboxOverflowStrategy, MessagePriorityGenerator, envelope::Envelope, message_queue::MessageQueue,
  },
};

/// Priority generator that assigns priority based on the i32 payload value.
struct PayloadPriorityGenerator;

impl MessagePriorityGenerator for PayloadPriorityGenerator {
  fn priority(&self, message: &AnyMessage) -> i32 {
    message.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX)
  }
}

fn capacity(n: usize) -> NonZeroUsize {
  NonZeroUsize::new(n).expect("capacity must be greater than 0")
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(10)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(10), MailboxOverflowStrategy::DropNewest);

  let r1 = queue.enqueue(Envelope::new(AnyMessage::new(30_i32)));
  assert!(matches!(r1, Ok(EnqueueOutcome::Accepted)), "within-capacity must be Accepted, got {r1:?}");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 20);

  let third = queue.dequeue().expect("dequeue 3rd").into_payload();
  assert_eq!(*third.payload().downcast_ref::<i32>().expect("downcast"), 30);
}

#[test]
fn equal_priority_preserves_insertion_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(10)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new("first"))).expect("enqueue first");
  queue.enqueue(Envelope::new(AnyMessage::new("second"))).expect("enqueue second");
  queue.enqueue(Envelope::new(AnyMessage::new("third"))).expect("enqueue third");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<&str>().expect("downcast"), "first");

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<&str>().expect("downcast"), "second");

  let third = queue.dequeue().expect("dequeue 3rd").into_payload();
  assert_eq!(*third.payload().downcast_ref::<&str>().expect("downcast"), "third");
}

/// MB-H3: DropNewest overflow must surface the rejected envelope via
/// `SendError::Full(payload)` so the mailbox can route it to DeadLetters.
#[test]
fn drop_newest_rejects_when_full() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(2)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(2), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");
  assert_eq!(queue.number_of_messages(), 2);

  let result = queue.enqueue(Envelope::new(AnyMessage::new(5_i32)));
  let Err(SendError::Full(payload)) = result else {
    panic!("DropNewest overflow must return SendError::Full, got {result:?}");
  };
  assert_eq!(
    payload.payload().downcast_ref::<i32>().copied(),
    Some(5_i32),
    "rejected payload must be the incoming envelope (not an existing heap entry)",
  );
  assert_eq!(queue.number_of_messages(), 2);
}

/// MB-H3: DropOldest on a stable-priority heap must evict the heap top
/// (the next envelope to be dequeued, i.e. highest priority, breaking ties
/// by insertion order). The evicted envelope must be surfaced through
/// `EnqueueOutcome::Evicted(_)` so DeadLetters receive it.
#[test]
fn drop_oldest_returns_evicted_outcome_with_heap_top_envelope() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(2)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(2), MailboxOverflowStrategy::DropOldest);

  // ヒープ: {10, 30}。10 が top (最高優先度、値が小さいほど優先度が高い)。
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  assert_eq!(queue.number_of_messages(), 2);

  let result = queue.enqueue(Envelope::new(AnyMessage::new(20_i32)));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert_eq!(
    evicted.payload().downcast_ref::<i32>().copied(),
    Some(10_i32),
    "DropOldest on a stable-priority heap must evict the heap top",
  );
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 20);

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 30);

  assert!(queue.dequeue().is_none());
}

/// MB-H3: When every queued message has equal priority, the DropOldest
/// eviction target is the entry with the smallest insertion sequence — the
/// FIFO "oldest" envelope. That evicted envelope must be carried by
/// `Evicted(_)` for DL forwarding.
#[test]
fn drop_oldest_with_equal_priority_evicts_earliest_and_returns_evicted() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(2)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(2), MailboxOverflowStrategy::DropOldest);

  // 全エントリは default priority i32::MAX (非 i32 payload) となり、優先度が等しい。
  queue.enqueue(Envelope::new(AnyMessage::new("a"))).expect("enqueue a");
  queue.enqueue(Envelope::new(AnyMessage::new("b"))).expect("enqueue b");

  let result = queue.enqueue(Envelope::new(AnyMessage::new("c")));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert_eq!(
    evicted.payload().downcast_ref::<&str>().copied(),
    Some("a"),
    "equal-priority DropOldest must evict the earliest insertion ('a')",
  );
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<&str>().expect("downcast"), "b");

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<&str>().expect("downcast"), "c");
}

/// MB-H3: Grow keeps accepting envelopes past nominal capacity; every
/// enqueue must return `Accepted` — no DL emissions should occur on this
/// path.
#[test]
fn grow_ignores_capacity() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(2)),
  );
  let queue = BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(2), MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  let result = queue.enqueue(Envelope::new(AnyMessage::new(20_i32)));
  assert!(matches!(result, Ok(EnqueueOutcome::Accepted)), "Grow must keep reporting Accepted, got {result:?}");
  assert_eq!(queue.number_of_messages(), 3);

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
}

#[test]
fn clean_up_removes_all_messages() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(10)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_i32))).expect("enqueue 2");
  queue.clean_up();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = BoundedStablePriorityMessageQueueStateShared::new(
    BoundedStablePriorityMessageQueueState::with_capacity(capacity(10)),
  );
  let queue =
    BoundedStablePriorityMessageQueue::new(pgen, state_shared, capacity(10), MailboxOverflowStrategy::DropNewest);
  assert!(queue.dequeue().is_none());
}
