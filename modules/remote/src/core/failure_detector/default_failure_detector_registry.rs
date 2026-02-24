//! Default implementation of the failure detector registry.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::BTreeMap};

use super::{FailureDetector, FailureDetectorRegistry};

/// Default registry that creates failure detectors on demand via a factory closure.
///
/// Each resource key maps to its own [`FailureDetector`] instance created by the
/// factory provided at construction time.
pub struct DefaultFailureDetectorRegistry<A: Ord + Clone> {
  factory:   Box<dyn Fn() -> Box<dyn FailureDetector + Send> + Send>,
  detectors: BTreeMap<A, Box<dyn FailureDetector + Send>>,
}

impl<A: Ord + Clone> DefaultFailureDetectorRegistry<A> {
  /// Creates a registry with the given factory function.
  ///
  /// The factory is called each time a previously unseen resource records its
  /// first heartbeat.
  pub fn new(factory: Box<dyn Fn() -> Box<dyn FailureDetector + Send> + Send>) -> Self {
    Self { factory, detectors: BTreeMap::new() }
  }
}

impl<A: Ord + Clone> FailureDetectorRegistry<A> for DefaultFailureDetectorRegistry<A> {
  fn is_available(&self, resource: &A, now_ms: u64) -> bool {
    match self.detectors.get(resource) {
      | Some(detector) => detector.is_available(now_ms),
      | None => true,
    }
  }

  fn is_monitoring(&self, resource: &A) -> bool {
    match self.detectors.get(resource) {
      | Some(detector) => detector.is_monitoring(),
      | None => false,
    }
  }

  fn heartbeat(&mut self, resource: &A, now_ms: u64) {
    let detector = self.detectors.entry(resource.clone()).or_insert_with(|| (self.factory)());
    detector.heartbeat(now_ms);
  }

  fn remove(&mut self, resource: &A) {
    self.detectors.remove(resource);
  }

  fn reset(&mut self) {
    self.detectors.clear();
  }
}
