use super::DemandTracker;

#[test]
fn request_zero_is_rejected() {
  let mut tracker = DemandTracker::new();
  let result = tracker.request(0);
  assert!(result.is_err());
}

#[test]
fn demand_is_consumed() {
  let mut tracker = DemandTracker::new();
  tracker.request(2).expect("request");
  assert!(tracker.has_demand());
  tracker.consume(1).expect("consume");
  assert!(tracker.has_demand());
  tracker.consume(1).expect("consume");
  assert!(!tracker.has_demand());
}

#[test]
fn demand_saturates_to_unbounded() {
  let mut tracker = DemandTracker::new();
  tracker.request(u64::MAX - 1).expect("request");
  tracker.request(2).expect("request");
  assert!(tracker.demand().is_unbounded());
}
