use super::TickDriverError;

#[test]
fn display_matches_public_contract() {
  let cases = [
    (TickDriverError::SpawnFailed, "failed to spawn tick driver background task"),
    (TickDriverError::HandleUnavailable, "runtime handle not available"),
    (TickDriverError::UnsupportedEnvironment, "unsupported environment for tick driver auto-detection"),
    (TickDriverError::DriftExceeded, "tick drift exceeded allowed threshold"),
    (TickDriverError::DriverStopped, "tick driver has stopped unexpectedly"),
    (TickDriverError::UnsupportedRuntime, "runtime flavor is not supported by this tick driver"),
    (TickDriverError::InvalidResolution, "tick driver resolution is zero or too small for safe operation"),
  ];

  for (error, expected) in cases {
    assert_eq!(alloc::format!("{error}"), expected);
  }
}
