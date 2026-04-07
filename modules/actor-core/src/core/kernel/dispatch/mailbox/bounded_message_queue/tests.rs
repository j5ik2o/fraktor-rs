use core::num::NonZeroUsize;

use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    bounded_message_queue::BoundedMessageQueue, envelope::Envelope, message_queue::MessageQueue,
    overflow_strategy::MailboxOverflowStrategy,
  },
};

#[test]
fn should_enqueue_and_dequeue_within_capacity() {
  let cap = NonZeroUsize::new(3).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(3_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 3);
  assert!(queue.dequeue().is_some());
  assert_eq!(queue.number_of_messages(), 2);
}

#[test]
fn should_reject_when_full_with_drop_newest() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();

  let result = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  assert!(result.is_err());
}

#[test]
fn should_drop_oldest_when_full() {
  let cap = NonZeroUsize::new(2).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(3_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 2);
}

#[test]
fn should_clean_up_all_messages() {
  let cap = NonZeroUsize::new(10).unwrap();
  let queue = BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest);

  for i in 0..5_u32 {
    queue.enqueue(Envelope::new(AnyMessage::new(i))).unwrap();
  }

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
}
