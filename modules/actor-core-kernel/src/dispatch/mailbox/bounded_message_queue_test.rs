use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};
use portable_atomic::{AtomicU64, Ordering};

use crate::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{
    EnqueueOutcome, MailboxClock, bounded_message_queue::BoundedMessageQueue, envelope::Envelope,
    message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
  },
};

fn fixed_zero_clock() -> MailboxClock {
  let closure: Box<dyn Fn() -> Duration + Send + Sync> = Box::new(|| Duration::ZERO);
  ArcShared::from_boxed(closure)
}

fn stepping_clock() -> MailboxClock {
  let tick = ArcShared::new(AtomicU64::new(0));
  let closure: Box<dyn Fn() -> Duration + Send + Sync> = Box::new(move || {
    let millis = tick.fetch_add(1, Ordering::SeqCst);
    Duration::from_millis(millis)
  });
  ArcShared::from_boxed(closure)
}

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

/// MB-H3: DropNewest overflow must surface `EnqueueOutcome::Rejected(payload)`
/// so the mailbox layer can route the rejected envelope to the dead-letter
/// destination with reason `MailboxFull`. The Pekko contract is "enqueue is
/// void-on-success" — the queue does not raise an error for overflow.
#[test]
fn should_reject_when_full_with_drop_newest() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue 2");

  // 拒否された envelope は識別可能な payload を保持しており、mailbox 層が
  // 情報を失うことなく DeadLetters へ転送できる。
  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Ok(EnqueueOutcome::Rejected(rejected)) = result else {
    panic!("DropNewest overflow must return Ok(Rejected(_)), got {result:?}");
  };
  assert_eq!(
    rejected.payload().downcast_ref::<u32>().copied(),
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
fn push_timeout_rejects_full_queue_without_drop_oldest_eviction() {
  let cap = NonZeroUsize::new(1).unwrap();
  let queue = BoundedMessageQueue::new_with_push_timeout(cap, MailboxOverflowStrategy::DropOldest, Duration::ZERO);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue first");

  let clock = fixed_zero_clock();
  let result = queue.enqueue_with_mailbox_clock(Envelope::new(AnyMessage::new(2_u32)), Some(&clock));
  let error = result.expect_err("zero push timeout must time out the incoming envelope without eviction");
  assert!(matches!(error.error(), SendError::Timeout(_)));
  assert_eq!(error.error().message().payload().downcast_ref::<u32>().copied(), Some(2_u32));

  let retained = queue.dequeue().expect("dequeue retained");
  assert_eq!(retained.payload().downcast_ref::<u32>().copied(), Some(1_u32));
  assert!(queue.dequeue().is_none());
}

#[test]
fn push_timeout_without_clock_falls_back_to_overflow_strategy() {
  let cap = NonZeroUsize::new(1).unwrap();
  let queue =
    BoundedMessageQueue::new_with_push_timeout(cap, MailboxOverflowStrategy::DropOldest, Duration::from_secs(1));

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue first");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  assert!(matches!(result, Ok(EnqueueOutcome::Evicted(_))), "{result:?}");

  let retained = queue.dequeue().expect("dequeue retained");
  assert_eq!(retained.payload().downcast_ref::<u32>().copied(), Some(2_u32));
}

#[test]
fn push_timeout_accepts_when_queue_has_room_with_clock() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue =
    BoundedMessageQueue::new_with_push_timeout(cap, MailboxOverflowStrategy::DropNewest, Duration::from_secs(1));
  let clock = fixed_zero_clock();

  let result = queue.enqueue_with_mailbox_clock(Envelope::new(AnyMessage::new(1_u32)), Some(&clock));

  assert!(matches!(result, Ok(EnqueueOutcome::Accepted)));
  assert_eq!(queue.number_of_messages(), 1);
}

#[test]
fn push_timeout_retries_until_deadline_when_queue_stays_full() {
  let cap = NonZeroUsize::new(1).unwrap();
  let queue =
    BoundedMessageQueue::new_with_push_timeout(cap, MailboxOverflowStrategy::DropNewest, Duration::from_millis(2));

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue first");

  let clock = stepping_clock();
  let error = queue
    .enqueue_with_mailbox_clock(Envelope::new(AnyMessage::new(2_u32)), Some(&clock))
    .expect_err("push timeout must eventually expire");

  assert!(matches!(error.error(), SendError::Timeout(_)));
  assert_eq!(error.error().message().payload().downcast_ref::<u32>().copied(), Some(2_u32));
}

#[test]
fn push_timeout_surfaces_queue_errors() {
  let cap = NonZeroUsize::new(1).unwrap();
  let queue =
    BoundedMessageQueue::new_with_push_timeout(cap, MailboxOverflowStrategy::DropNewest, Duration::from_secs(1));
  queue.handle.state.with_write(|state| state.queue.close().expect("close queue"));

  let clock = fixed_zero_clock();
  let error = queue
    .enqueue_with_mailbox_clock(Envelope::new(AnyMessage::new(1_u32)), Some(&clock))
    .expect_err("closed queue must surface an enqueue error");

  assert!(matches!(error.error(), SendError::Closed(_)));
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
