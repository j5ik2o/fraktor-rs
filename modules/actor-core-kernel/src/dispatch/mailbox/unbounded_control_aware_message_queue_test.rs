use crate::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    envelope::Envelope, message_queue::MessageQueue,
    unbounded_control_aware_message_queue::UnboundedControlAwareMessageQueue,
  },
};

#[test]
fn should_enqueue_and_dequeue_normal_messages() {
  let queue = UnboundedControlAwareMessageQueue::new();
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().unwrap().into_payload();
  assert_eq!(*first.payload().downcast_ref::<u32>().unwrap(), 1);

  let second = queue.dequeue().unwrap().into_payload();
  assert_eq!(*second.payload().downcast_ref::<u32>().unwrap(), 2);

  assert!(queue.dequeue().is_none());
}

#[test]
fn should_prioritise_control_messages() {
  let queue = UnboundedControlAwareMessageQueue::new();

  // Enqueue normal messages first.
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();

  // Enqueue a control message after normal messages.
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 3);

  // Control message should be dequeued first.
  let first = queue.dequeue().unwrap().into_payload();
  assert!(first.is_control());
  assert_eq!(*first.payload().downcast_ref::<u32>().unwrap(), 99);

  // Then normal messages in FIFO order.
  let second = queue.dequeue().unwrap().into_payload();
  assert!(!second.is_control());
  assert_eq!(*second.payload().downcast_ref::<u32>().unwrap(), 1);

  let third = queue.dequeue().unwrap().into_payload();
  assert!(!third.is_control());
  assert_eq!(*third.payload().downcast_ref::<u32>().unwrap(), 2);

  assert!(queue.dequeue().is_none());
}

#[test]
fn should_maintain_fifo_among_control_messages() {
  let queue = UnboundedControlAwareMessageQueue::new();

  queue.enqueue(Envelope::new(AnyMessage::control(10_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(20_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(30_u32))).unwrap();

  for expected in [10_u32, 20, 30] {
    let msg = queue.dequeue().unwrap().into_payload();
    assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), expected);
  }
  assert!(queue.dequeue().is_none());
}

#[test]
fn should_interleave_control_and_normal_correctly() {
  let queue = UnboundedControlAwareMessageQueue::new();

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(100_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(200_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(3_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 5);

  // Control messages first (FIFO): 100, 200
  let msg = queue.dequeue().unwrap().into_payload();
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 100);

  let msg = queue.dequeue().unwrap().into_payload();
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 200);

  // Then normal messages (FIFO): 1, 2, 3
  for expected in [1_u32, 2, 3] {
    let msg = queue.dequeue().unwrap().into_payload();
    assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), expected);
  }
  assert!(queue.dequeue().is_none());
}

#[test]
fn should_return_none_when_empty() {
  let queue = UnboundedControlAwareMessageQueue::new();
  assert!(queue.dequeue().is_none());
  assert!(!queue.has_messages());
  assert_eq!(queue.number_of_messages(), 0);
}

#[test]
fn should_clean_up_both_queues() {
  let queue = UnboundedControlAwareMessageQueue::new();
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(2_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(3_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 3);

  queue.clean_up();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn should_count_both_queues() {
  let queue = UnboundedControlAwareMessageQueue::new();

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(2_u32))).unwrap();

  assert_eq!(queue.number_of_messages(), 2);
  assert!(queue.has_messages());
}

#[test]
fn should_drain_control_before_normal() {
  let queue = UnboundedControlAwareMessageQueue::new();

  // Only control messages.
  queue.enqueue(Envelope::new(AnyMessage::control(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(2_u32))).unwrap();

  let first = queue.dequeue().unwrap().into_payload();
  assert_eq!(*first.payload().downcast_ref::<u32>().unwrap(), 1);

  // Add a normal message while control is still present.
  queue.enqueue(Envelope::new(AnyMessage::new(99_u32))).unwrap();

  // Control should still come first.
  let second = queue.dequeue().unwrap().into_payload();
  assert_eq!(*second.payload().downcast_ref::<u32>().unwrap(), 2);

  // Then the normal message.
  let third = queue.dequeue().unwrap().into_payload();
  assert_eq!(*third.payload().downcast_ref::<u32>().unwrap(), 99);

  assert!(queue.dequeue().is_none());
}
