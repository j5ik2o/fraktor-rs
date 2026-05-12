use super::StreamRefHandoff;
use crate::StreamError;

#[test]
fn poll_or_drain_returns_values_then_completion() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert_eq!(handoff.complete(), 2);

  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(Some(20_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(None));
}

#[test]
fn poll_or_drain_propagates_failure() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.fail(StreamError::Failed);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::Failed));
}

#[test]
fn offer_rejects_values_beyond_configured_buffer_capacity() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.configure_buffer_capacity(1);

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Err(StreamError::BufferOverflow));
}
