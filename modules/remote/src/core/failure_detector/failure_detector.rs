/// Trait for monitoring the availability of a single resource through heartbeats.
pub trait FailureDetector {
  /// Returns `true` if the resource is considered available.
  fn is_available(&self, now_ms: u64) -> bool;

  /// Returns `true` if at least one heartbeat has been recorded.
  fn is_monitoring(&self) -> bool;

  /// Records a heartbeat arrival from the monitored resource.
  fn heartbeat(&mut self, now_ms: u64);
}
