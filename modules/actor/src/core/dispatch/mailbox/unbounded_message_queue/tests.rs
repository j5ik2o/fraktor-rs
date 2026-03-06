use crate::core::{
  dispatch::mailbox::{message_queue::MessageQueue, unbounded_message_queue::UnboundedMessageQueue},
  messaging::AnyMessage,
};

#[test]
fn should_enqueue_and_dequeue_messages() {
  let queue = UnboundedMessageQueue::new();
  let msg = AnyMessage::new(42_u32);
  queue.enqueue(msg).unwrap();

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
    queue.enqueue(AnyMessage::new(i)).unwrap();
  }
  assert_eq!(queue.number_of_messages(), 5);

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
}
