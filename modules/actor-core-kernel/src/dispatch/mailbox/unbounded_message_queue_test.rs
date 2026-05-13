use crate::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{
    envelope::Envelope, message_queue::MessageQueue, unbounded_message_queue::UnboundedMessageQueue,
  },
};

#[test]
fn should_enqueue_and_dequeue_messages() {
  let queue = UnboundedMessageQueue::new();
  let msg = AnyMessage::new(42_u32);
  queue.enqueue(Envelope::new(msg)).unwrap();

  assert_eq!(queue.number_of_messages(), 1);
  assert!(queue.has_messages());

  let dequeued = queue.dequeue();
  assert!(dequeued.is_some());
  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}

#[test]
fn should_return_none_when_empty() {
  let queue = UnboundedMessageQueue::new();
  assert!(queue.dequeue().is_none());
  assert!(!queue.has_messages());
}

#[test]
fn should_clean_up_all_messages() {
  let queue = UnboundedMessageQueue::new();
  for i in 0..5_u32 {
    queue.enqueue(Envelope::new(AnyMessage::new(i))).unwrap();
  }
  assert_eq!(queue.number_of_messages(), 5);

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
}

#[test]
fn should_not_require_mailbox_put_lock_for_enqueue() {
  let queue = UnboundedMessageQueue::new();

  assert!(!queue.requires_put_lock_for_enqueue());
}

#[test]
fn should_reject_enqueue_after_cleanup_closes_queue() {
  let queue = UnboundedMessageQueue::new();
  queue.clean_up();

  let result = queue.enqueue(Envelope::new(AnyMessage::new("closed")));
  let error = result.expect_err("closed unbounded queue must reject enqueue");

  assert!(matches!(error.error(), SendError::Closed(_)));
  assert_eq!(queue.number_of_messages(), 0);
}
