use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared, EnqueueOutcome, MailboxOverflowStrategy,
    MessagePriorityGenerator, envelope::Envelope, message_queue::MessageQueue,
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

fn queue(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
) -> BoundedPriorityMessageQueue {
  let state_shared =
    BoundedPriorityMessageQueueStateShared::new(BoundedPriorityMessageQueueState::with_capacity(capacity));
  BoundedPriorityMessageQueue::new(generator, state_shared, capacity, overflow)
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

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

/// MB-H3: DropNewest overflow must expose the rejected envelope via
/// `EnqueueOutcome::Rejected(payload)` so the mailbox can route it to
/// DeadLetters. The Pekko contract is "enqueue is void-on-success".
#[test]
fn drop_newest_rejects_when_full() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");
  assert_eq!(queue.number_of_messages(), 2);

  let result = queue.enqueue(Envelope::new(AnyMessage::new(5_i32)));
  let Ok(EnqueueOutcome::Rejected(rejected)) = result else {
    panic!("DropNewest overflow must return Ok(Rejected(_)), got {result:?}");
  };
  assert_eq!(
    rejected.payload().downcast_ref::<i32>().copied(),
    Some(5_i32),
    "rejected payload must be the incoming envelope (not an existing heap entry)",
  );
  assert_eq!(queue.number_of_messages(), 2);
}

/// MB-H3: Pekko's BoundedPriorityMailbox removes the **heap top** on
/// DropOldest (the next message that would be dequeued — i.e. the
/// highest-priority entry). The evicted envelope must be returned via
/// `EnqueueOutcome::Evicted(_)` so the mailbox layer can forward it to
/// DeadLetters instead of silently dropping it.
#[test]
fn drop_oldest_returns_evicted_outcome_with_heap_top_envelope() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::DropOldest);

  // 優先度 10 と 30 でヒープを埋める。heap top は 10 (値が小さいほど優先度が高い)。
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  assert_eq!(queue.number_of_messages(), 2);

  // overflow を誘発。heap top (10) が evict されて Outcome として露出されなければならない。
  let result = queue.enqueue(Envelope::new(AnyMessage::new(20_i32)));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert_eq!(
    evicted.payload().downcast_ref::<i32>().copied(),
    Some(10_i32),
    "DropOldest on a priority heap must evict the heap top (highest-priority = 10)",
  );
  assert_eq!(queue.number_of_messages(), 2);

  // evict 後のヒープは {20, 30}。20 が最初に dequeue されなければならない。
  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 20);

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 30);

  assert!(queue.dequeue().is_none());
}

/// MB-H3: Grow ignores capacity, so every enqueue returns `Accepted` —
/// no eviction and therefore no DL entries should be generated.
#[test]
fn grow_ignores_capacity() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::Grow);

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
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_i32))).expect("enqueue 2");
  queue.clean_up();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);
  assert!(queue.dequeue().is_none());
}
