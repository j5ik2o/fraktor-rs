extern crate std;

use super::*;

#[test]
fn ring_buffer_offer_poll() {
  let mut buffer = RingBuffer::new(2).with_dynamic(false);
  buffer.offer_mut(1).unwrap();
  buffer.offer_mut(2).unwrap();
  assert_eq!(buffer.offer_mut(3), Err(QueueError::Full(3)));

  assert_eq!(buffer.poll_mut().unwrap(), Some(1));
  assert_eq!(buffer.poll_mut().unwrap(), Some(2));
  assert_eq!(buffer.poll_mut().unwrap(), None);
}

#[test]
fn ring_buffer_grows_when_dynamic() {
  let mut buffer = RingBuffer::new(1);
  buffer.offer_mut(1).unwrap();
  buffer.offer_mut(2).unwrap();
  assert_eq!(buffer.len().to_usize(), 2);
  assert!(buffer.capacity().is_limitless());
}
