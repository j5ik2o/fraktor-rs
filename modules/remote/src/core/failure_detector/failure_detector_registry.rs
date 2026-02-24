/// Registry managing per-resource failure detectors.
///
/// New resources are implicitly registered when [`heartbeat`](Self::heartbeat)
/// is first called with a given resource key.
pub trait FailureDetectorRegistry<A> {
  /// Returns `true` if the resource is considered up and healthy.
  ///
  /// For unregistered resources this returns `true`.
  fn is_available(&self, resource: &A, now_ms: u64) -> bool;

  /// Returns `true` if the failure detector has received any heartbeats
  /// for the given resource.
  fn is_monitoring(&self, resource: &A) -> bool;

  /// Records a heartbeat for the resource. If the resource is not yet
  /// registered this call implicitly creates a detector for it.
  fn heartbeat(&mut self, resource: &A, now_ms: u64);

  /// Removes heartbeat management for the resource.
  fn remove(&mut self, resource: &A);

  /// Removes all resources and any associated failure detector state.
  fn reset(&mut self);
}
