use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use super::StreamBuffer;
use crate::core::stream_error::StreamError;

#[test]
fn buffer_rejects_when_full() {
  let mut buffer = StreamBuffer::new(1, OverflowPolicy::Block);
  assert!(buffer.offer(10_u32).is_ok());
  assert_eq!(buffer.offer(20_u32), Err(StreamError::BufferFull));
}

#[test]
fn buffer_poll_empty_returns_error() {
  let mut buffer = StreamBuffer::<u32>::new(1, OverflowPolicy::Block);
  assert_eq!(buffer.poll(), Err(StreamError::BufferEmpty));
}
