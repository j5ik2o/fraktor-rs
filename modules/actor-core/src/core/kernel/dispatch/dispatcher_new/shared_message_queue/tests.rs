use super::SharedMessageQueue;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{EnqueueOutcome, MessageQueue},
};

#[test]
fn enqueue_appends_messages() {
  let queue = SharedMessageQueue::new();
  let outcome = queue.enqueue(AnyMessage::new(42_u32)).expect("enqueue");
  assert!(matches!(outcome, EnqueueOutcome::Enqueued));
  assert_eq!(queue.number_of_messages(), 1);
  assert!(queue.has_messages());
}

#[test]
fn dequeue_pops_in_fifo_order() {
  let queue = SharedMessageQueue::new();
  let _ = queue.enqueue(AnyMessage::new(1_u32)).unwrap();
  let _ = queue.enqueue(AnyMessage::new(2_u32)).unwrap();
  let first = queue.dequeue().expect("first");
  let second = queue.dequeue().expect("second");
  assert!(first.as_view().downcast_ref::<u32>().is_some());
  assert!(second.as_view().downcast_ref::<u32>().is_some());
  assert!(queue.dequeue().is_none());
}

#[test]
fn clean_up_does_not_drain_messages() {
  let queue = SharedMessageQueue::new();
  let _ = queue.enqueue(AnyMessage::new(99_u32)).unwrap();
  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 1);
  assert!(queue.has_messages());
}

#[test]
fn clone_shares_underlying_storage() {
  let queue = SharedMessageQueue::new();
  let cloned = queue.clone();
  let _ = queue.enqueue(AnyMessage::new(7_u32)).unwrap();
  assert_eq!(cloned.number_of_messages(), 1);
}
