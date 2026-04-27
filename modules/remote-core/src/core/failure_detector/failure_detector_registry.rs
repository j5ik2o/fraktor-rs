//! `FailureDetectorRegistry` trait for resource-scoped detectors.

/// Manages failure detectors keyed by monitored resource.
///
/// New resources are registered implicitly when
/// [`heartbeat`](Self::heartbeat) is first called for that resource.
pub trait FailureDetectorRegistry<K> {
  /// Returns `true` when the resource is considered available.
  ///
  /// Unregistered resources are considered available.
  #[must_use]
  fn is_available(&self, resource: &K, now_ms: u64) -> bool;

  /// Returns `true` after the resource has received at least one heartbeat.
  #[must_use]
  fn is_monitoring(&self, resource: &K) -> bool;

  /// Records a heartbeat for the resource, registering it on first use.
  fn heartbeat(&mut self, resource: &K, now_ms: u64);

  /// Removes heartbeat management for the resource.
  fn remove(&mut self, resource: &K);

  /// Removes all resources and detector state.
  fn reset(&mut self);
}
