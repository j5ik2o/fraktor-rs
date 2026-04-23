use core::num::NonZeroUsize;

use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    EnqueueOutcome, bounded_control_aware_message_queue::BoundedControlAwareMessageQueue, envelope::Envelope,
    message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
  },
};

/// spec Requirement 2 Scenario "control envelope が normal より先に dequeue される":
/// control envelope は normal より優先的に dequeue される。
#[test]
fn control_envelopes_are_dequeued_before_normal() {
  let cap = NonZeroUsize::new(10).unwrap();
  let queue = BoundedControlAwareMessageQueue::new(cap, MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue normal_B");
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control_X");

  assert_eq!(queue.number_of_messages(), 3);

  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert!(first.is_control());
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));

  let second = queue.dequeue().expect("dequeue 2").into_payload();
  assert!(!second.is_control());
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(1_u32));

  let third = queue.dequeue().expect("dequeue 3").into_payload();
  assert!(!third.is_control());
  assert_eq!(third.payload().downcast_ref::<u32>().copied(), Some(2_u32));
}

/// spec Requirement 2 Scenario "DropOldest は normal queue の front を優先 evict する":
/// DropOldest + capacity 超過時、normal queue の front が evict される。
#[test]
fn drop_oldest_evicts_normal_front_first() {
  let cap = NonZeroUsize::new(3).unwrap();
  let queue = BoundedControlAwareMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue normal_B");
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control_X");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  let Ok(EnqueueOutcome::Evicted(evicted)) = result else {
    panic!("DropOldest overflow must return Ok(Evicted(_)), got {result:?}");
  };
  assert!(!evicted.payload().is_control());
  assert_eq!(evicted.payload().downcast_ref::<u32>().copied(), Some(1_u32));
  assert_eq!(queue.number_of_messages(), 3);

  // dequeue 順: control_X → normal_B → normal_C
  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert!(first.is_control());
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));

  let second = queue.dequeue().expect("dequeue 2").into_payload();
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(2_u32));

  let third = queue.dequeue().expect("dequeue 3").into_payload();
  assert_eq!(third.payload().downcast_ref::<u32>().copied(), Some(3_u32));
}

/// spec Requirement 2 Scenario "DropOldest 下で normal queue が空なら control envelope を Reject
/// する": normal queue が空で capacity 超過時は control drop を避けるため `Rejected` を返す。
#[test]
fn drop_oldest_rejects_control_when_normal_queue_is_empty() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedControlAwareMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::control(10_u32))).expect("enqueue control_X");
  queue.enqueue(Envelope::new(AnyMessage::control(20_u32))).expect("enqueue control_Y");

  let result = queue.enqueue(Envelope::new(AnyMessage::control(30_u32)));
  let Ok(EnqueueOutcome::Rejected(rejected)) = result else {
    panic!("DropOldest with empty normal queue must return Ok(Rejected(_)), got {result:?}");
  };
  assert!(rejected.payload().is_control());
  assert_eq!(rejected.payload().downcast_ref::<u32>().copied(), Some(30_u32));
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(10_u32));
  let second = queue.dequeue().expect("dequeue 2").into_payload();
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(20_u32));
}

/// spec Requirement 2 Scenario "DropNewest で capacity 超過時は到着 envelope を拒否する":
/// DropNewest では到着 envelope を `Rejected` し、既存 entry は保持される。
#[test]
fn drop_newest_rejects_incoming_when_full() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedControlAwareMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control_X");

  let result = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  let Ok(EnqueueOutcome::Rejected(rejected)) = result else {
    panic!("DropNewest overflow must return Ok(Rejected(_)), got {result:?}");
  };
  assert!(!rejected.payload().is_control());
  assert_eq!(rejected.payload().downcast_ref::<u32>().copied(), Some(2_u32));
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert!(first.is_control());
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));
  let second = queue.dequeue().expect("dequeue 2").into_payload();
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(1_u32));
}

/// spec Requirement 2 Scenario "Grow strategy で capacity を超えた enqueue も受理する":
/// Grow では合計長が capacity を超えても全 enqueue が `Accepted` になる。
#[test]
fn grow_accepts_beyond_capacity_and_preserves_priority() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedControlAwareMessageQueue::new(cap, MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::control(10_u32))).expect("enqueue control_X");
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");

  // 追加 3 件で合計 5。Grow なので全て Accepted。
  for envelope in [
    Envelope::new(AnyMessage::new(2_u32)),
    Envelope::new(AnyMessage::new(3_u32)),
    Envelope::new(AnyMessage::control(20_u32)),
  ] {
    let result = queue.enqueue(envelope);
    assert!(matches!(result, Ok(EnqueueOutcome::Accepted)), "Grow must return Accepted, got {result:?}");
  }
  assert_eq!(queue.number_of_messages(), 5);

  // control_X, control_Y, normal_A, normal_B, normal_C の順
  let expected = [10_u32, 20, 1, 2, 3];
  for &value in &expected {
    let payload = queue.dequeue().expect("dequeue").into_payload();
    assert_eq!(payload.payload().downcast_ref::<u32>().copied(), Some(value));
  }
  assert!(queue.dequeue().is_none());
}
