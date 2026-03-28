use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use super::{StreamBuffer, StreamBufferConfig};

#[test]
fn buffer_rejects_overflow_when_blocking() {
  let config = StreamBufferConfig::new(1, OverflowPolicy::Block);
  let buffer = StreamBuffer::new(config);
  buffer.offer(1_u32).expect("offer");
  let result = buffer.offer(2_u32);
  assert!(result.is_err());
}

#[test]
fn buffer_poll_returns_items_in_order() {
  let config = StreamBufferConfig::new(4, OverflowPolicy::Block);
  let buffer = StreamBuffer::new(config);
  buffer.offer(10_u32).expect("offer");
  buffer.offer(11_u32).expect("offer");
  assert_eq!(buffer.poll().expect("poll"), 10);
  assert_eq!(buffer.poll().expect("poll"), 11);
}
