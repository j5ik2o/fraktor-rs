use crate::core::kernel::actor::{
  actor_ref::ActorRef,
  messaging::{AnyMessage, MessageBuffer},
};

#[test]
fn empty_buffer_is_empty() {
  let buf = MessageBuffer::empty();
  assert!(buf.is_empty());
  assert_eq!(buf.size(), 0);
  assert!(buf.head().is_none());
}

#[test]
fn append_increases_size() {
  let mut buf = MessageBuffer::empty();
  buf.append(AnyMessage::new(1_u32), ActorRef::null());
  assert!(!buf.is_empty());
  assert_eq!(buf.size(), 1);
}

#[test]
fn fifo_order_is_preserved() {
  let mut buf = MessageBuffer::empty();
  buf.append(AnyMessage::new(10_u32), ActorRef::null());
  buf.append(AnyMessage::new(20_u32), ActorRef::null());
  buf.append(AnyMessage::new(30_u32), ActorRef::null());

  let (msg, _) = buf.head().expect("non-empty");
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 10);

  buf.drop_head();
  let (msg, _) = buf.head().expect("non-empty");
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 20);

  buf.drop_head();
  let (msg, _) = buf.head().expect("non-empty");
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 30);

  buf.drop_head();
  assert!(buf.is_empty());
}

#[test]
fn drop_head_on_empty_is_noop() {
  let mut buf = MessageBuffer::empty();
  buf.drop_head();
  assert!(buf.is_empty());
}

#[test]
fn for_each_visits_all_elements() {
  let mut buf = MessageBuffer::empty();
  buf.append(AnyMessage::new(1_u32), ActorRef::null());
  buf.append(AnyMessage::new(2_u32), ActorRef::null());
  buf.append(AnyMessage::new(3_u32), ActorRef::null());

  let mut values = alloc::vec::Vec::new();
  buf.for_each(|msg, _| {
    values.push(*msg.payload().downcast_ref::<u32>().unwrap());
  });
  assert_eq!(values, alloc::vec![1, 2, 3]);
}

#[test]
fn retain_filters_elements() {
  let mut buf = MessageBuffer::empty();
  buf.append(AnyMessage::new(1_u32), ActorRef::null());
  buf.append(AnyMessage::new(2_u32), ActorRef::null());
  buf.append(AnyMessage::new(3_u32), ActorRef::null());
  buf.append(AnyMessage::new(4_u32), ActorRef::null());

  buf.retain(|msg, _| {
    let val = msg.payload().downcast_ref::<u32>().unwrap();
    *val % 2 == 0
  });

  assert_eq!(buf.size(), 2);
  let (msg, _) = buf.head().expect("non-empty");
  assert_eq!(*msg.payload().downcast_ref::<u32>().unwrap(), 2);
}

#[test]
fn default_creates_empty_buffer() {
  let buf = MessageBuffer::default();
  assert!(buf.is_empty());
}
