use crate::core::kernel::{
  dispatch::mailbox::{message_queue::MessageQueue, unbounded_deque_message_queue::UnboundedDequeMessageQueue},
  messaging::AnyMessage,
};

#[test]
fn should_enqueue_and_dequeue_messages() {
  let queue = UnboundedDequeMessageQueue::new();
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
  let queue = UnboundedDequeMessageQueue::new();
  assert!(queue.dequeue().is_none());
  assert!(!queue.has_messages());
}

#[test]
fn should_clean_up_all_messages() {
  let queue = UnboundedDequeMessageQueue::new();
  for i in 0..5_u32 {
    queue.enqueue(AnyMessage::new(i)).unwrap();
  }
  assert_eq!(queue.number_of_messages(), 5);

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
}

#[test]
fn should_expose_deque_capability() {
  let queue = UnboundedDequeMessageQueue::new();
  assert!(queue.as_deque().is_some());
}

#[test]
fn should_prepend_via_enqueue_first() {
  let queue = UnboundedDequeMessageQueue::new();

  queue.enqueue(AnyMessage::new(1_u32)).unwrap();
  queue.enqueue(AnyMessage::new(2_u32)).unwrap();
  queue.enqueue(AnyMessage::new(3_u32)).unwrap();

  // Prepend a message to the front.
  queue.as_deque().unwrap().enqueue_first(AnyMessage::new(0_u32)).unwrap();

  assert_eq!(queue.number_of_messages(), 4);

  // The prepended message should be dequeued first.
  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<u32>().unwrap(), 0);

  let second = queue.dequeue().unwrap();
  assert_eq!(*second.payload().downcast_ref::<u32>().unwrap(), 1);

  let third = queue.dequeue().unwrap();
  assert_eq!(*third.payload().downcast_ref::<u32>().unwrap(), 2);

  let fourth = queue.dequeue().unwrap();
  assert_eq!(*fourth.payload().downcast_ref::<u32>().unwrap(), 3);
}

#[test]
fn should_maintain_fifo_order_for_normal_enqueue() {
  let queue = UnboundedDequeMessageQueue::new();

  for i in 0..5_u32 {
    queue.enqueue(AnyMessage::new(i)).unwrap();
  }

  for expected in 0..5_u32 {
    let msg = queue.dequeue().unwrap();
    assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), expected);
  }
}

#[test]
fn should_interleave_enqueue_and_enqueue_first() {
  let queue = UnboundedDequeMessageQueue::new();

  queue.enqueue(AnyMessage::new(10_u32)).unwrap();
  queue.enqueue(AnyMessage::new(20_u32)).unwrap();
  queue.as_deque().unwrap().enqueue_first(AnyMessage::new(5_u32)).unwrap();
  queue.enqueue(AnyMessage::new(30_u32)).unwrap();
  queue.as_deque().unwrap().enqueue_first(AnyMessage::new(1_u32)).unwrap();

  // Expected order: 1, 5, 10, 20, 30
  let expected = [1_u32, 5, 10, 20, 30];
  for &val in &expected {
    let msg = queue.dequeue().unwrap();
    assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), val);
  }
  assert!(queue.dequeue().is_none());
}
