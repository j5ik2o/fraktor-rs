use core::num::NonZeroU64;

use super::StreamRefHandoff;
use crate::{StreamError, stage::CancellationCause};

#[test]
fn poll_or_drain_returns_values_then_completion() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert_eq!(handoff.complete(), 2);

  assert_eq!(handoff.record_cumulative_demand(), Ok(()));
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
  assert_eq!(handoff.record_cumulative_demand(), Ok(()));
  assert_eq!(handoff.poll_or_drain(), Ok(Some(20_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(None));
}

#[test]
fn poll_or_drain_waits_for_cumulative_demand_before_delivering_value() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::WouldBlock));
  assert_eq!(handoff.record_cumulative_demand(), Ok(()));
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
}

#[test]
fn completion_waits_behind_pending_elements_until_demand_arrives() {
  let handoff = StreamRefHandoff::new();

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.complete(), 1);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::WouldBlock));
  assert_eq!(handoff.record_cumulative_demand(), Ok(()));
  assert_eq!(handoff.poll_or_drain(), Ok(Some(10_u32)));
  assert_eq!(handoff.poll_or_drain(), Ok(None));
}

#[test]
fn poll_or_drain_propagates_failure() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.fail(StreamError::Failed);

  assert_eq!(handoff.poll_or_drain(), Err(StreamError::Failed));
}

#[test]
fn close_for_cancel_is_observed_as_cancellation_not_completion() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.close_for_cancel();

  assert_eq!(
    handoff.poll_or_drain(),
    Err(StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() })
  );
}

#[test]
fn close_for_cancel_rejects_additional_publication() {
  let handoff = StreamRefHandoff::<u32>::new();

  handoff.close_for_cancel();

  assert_eq!(
    handoff.offer(10_u32),
    Err(StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() })
  );
}

#[test]
fn offer_rejects_values_beyond_configured_buffer_capacity() {
  let handoff = StreamRefHandoff::<u32>::new();
  handoff.configure_buffer_capacity(1);

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.offer(20_u32), Err(StreamError::BufferOverflow));
}

#[test]
fn stale_cumulative_demand_after_delivered_sequence_is_ignored() {
  let handoff = StreamRefHandoff::new();
  let demand = NonZeroU64::new(1).expect("demand");

  assert_eq!(handoff.offer(10_u32), Ok(0));
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.drain_ready_protocols().expect("first drain").len(), 1);
  assert_eq!(handoff.record_cumulative_demand_from(0, demand), Ok(()));
  assert_eq!(handoff.offer(20_u32), Ok(1));
  assert!(handoff.drain_ready_protocols().expect("stale demand drain").is_empty());
  assert_eq!(handoff.record_cumulative_demand_from(1, demand), Ok(()));
  assert_eq!(handoff.drain_ready_protocols().expect("second drain").len(), 1);
}
