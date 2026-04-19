use core::num::NonZeroUsize;

use crate::core::kernel::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{
    EnqueueOutcome, bounded_message_queue::BoundedMessageQueue, envelope::Envelope, message_queue::MessageQueue,
    overflow_strategy::MailboxOverflowStrategy,
  },
};

#[test]
fn should_enqueue_and_dequeue_within_capacity() {
  let cap = NonZeroUsize::new(3).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  // Within capacity every successful enqueue must report `Accepted` —
  // `Evicted` is reserved for overflow paths only.
  let first = queue.enqueue(Envelope::new(AnyMessage::new(1_u32)));
  assert!(matches!(first, Ok(EnqueueOutcome::Accepted)), "within-capacity enqueue must report Accepted, got {first:?}");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue 2");
  queue.enqueue(Envelope::new(AnyMessage::new(3_u32))).expect("enqueue 3");

  assert_eq!(queue.number_of_messages(), 3);
  assert!(queue.dequeue().is_some());
  assert_eq!(queue.number_of_messages(), 2);
}

/// MB-H3: DropNewest overflow must surface `SendError::Full(payload)` so the
/// mailbox layer can route the rejected envelope to the dead-letter mailbox
/// with reason `MailboxFull`.
#[test]
fn should_reject_when_full_with_drop_newest() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue 2");

  // 拒否された envelope は識別可能な payload を保持しており、dispatcher が
  // 情報を失うことなく DeadLetters へ転送できる。
  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Err(SendError::Full(payload)) = result else {
    panic!("DropNewest overflow must return SendError::Full, got {result:?}");
  };
  assert_eq!(
    payload.payload().downcast_ref::<u32>().copied(),
    Some(3_u32),
    "rejected payload must be the incoming envelope",
  );
  assert_eq!(queue.number_of_messages(), 2, "queue length must stay at capacity");
}

/// MB-H3: DropOldest overflow must surface the evicted envelope via
/// `EnqueueOutcome::Evicted(_)` so the mailbox layer can route the evicted
/// payload to DeadLetters. Silent drops break observability.
#[test]
fn drop_oldest_returns_evicted_outcome_with_oldest_envelope() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue 2");

  // BoundedMessageQueue は FIFO — 「最古」は最初に挿入された envelope、
  // すなわち次に dequeue される対象。
  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert_eq!(
    evicted.payload().downcast_ref::<u32>().copied(),
    Some(1_u32),
    "DropOldest must evict the first-inserted envelope",
  );

  // 新しい envelope は受理される — overflow は到着メッセージを失わず、
  // 空きを作るために最古を evict するだけ。
  assert_eq!(queue.number_of_messages(), 2);
  let next = queue.dequeue().expect("dequeue");
  assert_eq!(next.payload().downcast_ref::<u32>().copied(), Some(2_u32));
  let second = queue.dequeue().expect("dequeue 2nd");
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(3_u32));
}

/// MB-H3: Grow strategy never evicts. Every enqueue past the nominal
/// capacity must still report `Accepted` — no DL entries should ever be
/// emitted by the mailbox layer on this path.
#[test]
fn grow_returns_accepted_even_past_capacity() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue 2");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  assert!(matches!(result, Ok(EnqueueOutcome::Accepted)), "Grow must keep reporting Accepted, got {result:?}");
  assert_eq!(queue.number_of_messages(), 3);
}

#[test]
fn should_clean_up_all_messages() {
  let cap = NonZeroUsize::new(10).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  for i in 0..5_u32 {
    queue.enqueue(Envelope::new(AnyMessage::new(i))).expect("enqueue");
  }

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
}
