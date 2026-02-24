/// Trait for monitoring the availability of a single resource through heartbeats.
///
/// A failure detector registers heartbeat events and decides the availability
/// of the monitored resource. Each implementation monitors exactly one resource;
/// use [`super::FailureDetectorRegistry`] for
/// multi-resource management.
pub trait FailureDetector {
  /// Returns `true` if the resource is considered up and healthy.
  ///
  /// For detectors that have not yet received any heartbeat, this returns `true`
  /// (unmanaged connections are treated as healthy).
  fn is_available(&self, now_ms: u64) -> bool;

  /// Returns `true` if at least one heartbeat has been recorded.
  fn is_monitoring(&self) -> bool;

  /// Records a heartbeat arrival from the monitored resource.
  fn heartbeat(&mut self, now_ms: u64);
}
