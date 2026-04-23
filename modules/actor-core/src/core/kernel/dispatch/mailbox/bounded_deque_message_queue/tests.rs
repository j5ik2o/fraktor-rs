use core::num::NonZeroUsize;

use crate::core::kernel::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{
    EnqueueOutcome, bounded_deque_message_queue::BoundedDequeMessageQueue, envelope::Envelope,
    message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
  },
};

/// spec Requirement 1 Scenario "Grow strategy で capacity を超えた enqueue も受理する":
/// Grow 戦略では capacity を超過しても `Accepted` を返す。
#[test]
fn grow_accepts_beyond_capacity() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::Grow);

  for i in 0..3_u32 {
    let result = queue.enqueue(Envelope::new(AnyMessage::new(i)));
    assert!(matches!(result, Ok(EnqueueOutcome::Accepted)), "enqueue {i} must be Accepted, got {result:?}");
  }
  assert_eq!(queue.number_of_messages(), 3);
}

/// spec Requirement 1 Scenario "DropNewest strategy で capacity 超過時は到着 envelope を拒否する":
/// DropNewest では到着 envelope を `Rejected` し、既存 entry は保持される。
#[test]
fn drop_newest_rejects_incoming_when_full() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue B");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Ok(EnqueueOutcome::Rejected(rejected)) = result else {
    panic!("DropNewest overflow must return Ok(Rejected(_)), got {result:?}");
  };
  assert_eq!(rejected.payload().downcast_ref::<u32>().copied(), Some(3_u32));
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue A");
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(1_u32));
  let second = queue.dequeue().expect("dequeue B");
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(2_u32));
}

/// spec Requirement 1 Scenario "DropOldest strategy で capacity 超過時は front を evict する":
/// DropOldest は front を evict し、到着 envelope を push_back する。
#[test]
fn drop_oldest_evicts_front_when_full() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue B");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert_eq!(evicted.payload().downcast_ref::<u32>().copied(), Some(1_u32));
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue B");
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(2_u32));
  let second = queue.dequeue().expect("dequeue C");
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(3_u32));
}

/// spec Requirement 1 Scenario "enqueue_first (front 挿入) が capacity と overflow に従う":
/// DropNewest で capacity 超過時に enqueue_first は `SendError::Full` を返す。
#[test]
fn enqueue_first_rejects_when_full_with_drop_newest() {
  let cap = NonZeroUsize::new(1).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");

  let deque = queue.as_deque().expect("deque capability");
  let result = deque.enqueue_first(Envelope::new(AnyMessage::new(2_u32)));
  let Err(SendError::Full(payload)) = result else {
    panic!("enqueue_first with DropNewest must return Err(SendError::Full), got {result:?}");
  };
  assert_eq!(payload.downcast_ref::<u32>().copied(), Some(2_u32));
  assert_eq!(queue.number_of_messages(), 1);
}

/// spec Requirement 1 Scenario "DropOldest 下の enqueue_first は capacity 超過なら Reject する
/// (Decision 2-c)": DropOldest + enqueue_first + capacity 超過時は evict せず `SendError::Full`
/// を返す。
#[test]
fn enqueue_first_rejects_when_full_with_drop_oldest_decision_2c() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue B");

  let deque = queue.as_deque().expect("deque capability");
  let result = deque.enqueue_first(Envelope::new(AnyMessage::new(3_u32)));
  let Err(SendError::Full(payload)) = result else {
    panic!("enqueue_first with DropOldest when full must return Err(SendError::Full), got {result:?}");
  };
  assert_eq!(payload.downcast_ref::<u32>().copied(), Some(3_u32));
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue A");
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(1_u32));
  let second = queue.dequeue().expect("dequeue B");
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(2_u32));
}

/// spec Requirement 1 Scenario "clean_up で全 envelope を破棄する":
/// `clean_up()` 後は `number_of_messages == 0` かつ `dequeue` が `None`。
#[test]
fn clean_up_removes_all_messages() {
  let cap = NonZeroUsize::new(10).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  for i in 0..5_u32 {
    queue.enqueue(Envelope::new(AnyMessage::new(i))).expect("enqueue");
  }
  assert_eq!(queue.number_of_messages(), 5);

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn grow_enqueue_first_bypasses_capacity() {
  // Grow は push_front でも capacity を無視する (spec: "Grow: capacity 無視で push_front")。
  let cap = NonZeroUsize::new(1).unwrap();
  let queue = BoundedDequeMessageQueue::new(cap, MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");

  let deque = queue.as_deque().expect("deque capability");
  deque.enqueue_first(Envelope::new(AnyMessage::new(0_u32))).expect("enqueue_first B");
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue front");
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(0_u32));
  let second = queue.dequeue().expect("dequeue back");
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(1_u32));
}
