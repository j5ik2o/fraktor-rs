use super::SchedulerError;

#[test]
fn display_matches_public_contract() {
  let cases = [
    (SchedulerError::InvalidDelay, "invalid delay or period"),
    (SchedulerError::ActorUnavailable, "actor cell unavailable"),
    (SchedulerError::Closed, "scheduler closed"),
    (SchedulerError::Backpressured, "scheduler backpressured"),
    (SchedulerError::CapacityExceeded, "scheduler capacity exceeded"),
    (SchedulerError::TaskRunCapacityExceeded, "scheduler task-run capacity exceeded"),
  ];

  for (error, expected) in cases {
    assert_eq!(alloc::format!("{error}"), expected);
  }
}
