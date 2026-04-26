//! Default failure detector registry.

use core::hash::Hash;

use ahash::RandomState;
use hashbrown::{HashMap, hash_map::Entry};

use crate::core::failure_detector::{FailureDetector, failure_detector_registry::FailureDetectorRegistry};

/// Registry that creates per-resource failure detectors on first heartbeat.
pub struct DefaultFailureDetectorRegistry<K, D, F>
where
  K: Eq + Hash + Clone,
  D: FailureDetector,
  F: Fn(&K) -> D, {
  factory:   F,
  detectors: HashMap<K, D, RandomState>,
}

impl<K, D, F> DefaultFailureDetectorRegistry<K, D, F>
where
  K: Eq + Hash + Clone,
  D: FailureDetector,
  F: Fn(&K) -> D,
{
  /// Creates a registry with a resource-aware detector factory.
  #[must_use]
  pub fn new(factory: F) -> Self {
    Self { factory, detectors: HashMap::with_hasher(RandomState::new()) }
  }

  fn detector_for(&mut self, resource: &K) -> &mut D {
    match self.detectors.entry(resource.clone()) {
      | Entry::Occupied(entry) => entry.into_mut(),
      | Entry::Vacant(entry) => entry.insert((self.factory)(resource)),
    }
  }
}

impl<K, D, F> FailureDetectorRegistry<K> for DefaultFailureDetectorRegistry<K, D, F>
where
  K: Eq + Hash + Clone,
  D: FailureDetector,
  F: Fn(&K) -> D,
{
  fn is_available(&self, resource: &K, now_ms: u64) -> bool {
    match self.detectors.get(resource) {
      | Some(detector) => detector.is_available(now_ms),
      | None => true,
    }
  }

  fn is_monitoring(&self, resource: &K) -> bool {
    match self.detectors.get(resource) {
      | Some(detector) => detector.is_monitoring(),
      | None => false,
    }
  }

  fn heartbeat(&mut self, resource: &K, now_ms: u64) {
    self.detector_for(resource).heartbeat(now_ms);
  }

  fn remove(&mut self, resource: &K) {
    // Removing an absent resource is a valid command outcome.
    let _removed = self.detectors.remove(resource);
  }

  fn reset(&mut self) {
    self.detectors.clear();
  }
}
