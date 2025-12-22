use super::DemandTracker;
use crate::core::{demand::Demand, stream_error::StreamError};

#[test]
fn request_rejects_zero() {
  let mut tracker = DemandTracker::new();
  assert_eq!(tracker.request(0), Err(StreamError::InvalidDemand));
}

#[test]
fn request_accumulates_and_consumes() {
  let mut tracker = DemandTracker::new();
  assert_eq!(tracker.request(2), Ok(Demand::Finite(2)));
  assert!(tracker.consume_one());
  assert_eq!(tracker.current(), Demand::Finite(1));
  assert!(tracker.consume_one());
  assert_eq!(tracker.current(), Demand::Finite(0));
  assert!(!tracker.consume_one());
}

#[test]
fn request_overflow_becomes_unbounded() {
  let mut tracker = DemandTracker::new();
  assert_eq!(tracker.request(u64::MAX), Ok(Demand::Finite(u64::MAX)));
  assert_eq!(tracker.request(1), Ok(Demand::Unbounded));
  assert!(tracker.consume_one());
  assert_eq!(tracker.current(), Demand::Unbounded);
}
