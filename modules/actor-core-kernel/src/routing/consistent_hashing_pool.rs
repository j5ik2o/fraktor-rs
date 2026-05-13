//! Pool router configuration using consistent-hashing routing logic.

#[cfg(test)]
#[path = "consistent_hashing_pool_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_utils_core_rs::sync::ArcShared;

use super::{
  Router, consistent_hashable_envelope::ConsistentHashableEnvelope,
  consistent_hashing_routing_logic::ConsistentHashingRoutingLogic, pool::Pool, router_config::RouterConfig,
};
use crate::actor::messaging::AnyMessage;

type HashKeyMapper = dyn Fn(&AnyMessage) -> u64 + Send + Sync;

/// Built-in mapper category carried by a consistent-hashing pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsistentHashingHashKeyMapperKind {
  /// Uses `ConsistentHashableEnvelope::hash_key()` and a fixed fallback key.
  EnvelopeHashKey,
  /// Uses an arbitrary process-local closure.
  CustomClosure,
}

/// Pool router that selects routees via consistent hashing.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.ConsistentHashingPool`.
///
/// Messages are mapped to a hash key by the provided `hash_key_mapper`, and
/// the routee with the highest rendezvous score is selected. This provides
/// stable routing — a given key consistently maps to the same routee as long
/// as the routee set does not change.
///
/// # Parameters not ported from Pekko
///
/// Pekko's `ConsistentHashingPool` exposes a `virtualNodesFactor` parameter
/// that controls how many virtual nodes each routee occupies on the sorted
/// hash ring. This constructor intentionally does **not** expose
/// `with_virtual_nodes_factor` because the underlying
/// [`ConsistentHashingRoutingLogic`] uses rendezvous hashing rather than a
/// sorted ring. Rendezvous hashing is uniform by construction and has no ring
/// to tune, so `virtualNodesFactor` would be a no-op knob that misleads users.
/// See the `# Design notes` section on [`ConsistentHashingRoutingLogic`] for
/// the full rationale.
pub struct ConsistentHashingPool {
  nr_of_instances:      usize,
  hash_key_mapper:      ArcShared<HashKeyMapper>,
  hash_key_mapper_kind: ConsistentHashingHashKeyMapperKind,
  router_dispatcher:    String,
}

impl ConsistentHashingPool {
  /// Creates a new consistent-hashing pool configuration.
  ///
  /// # Arguments
  ///
  /// * `nr_of_instances` - Number of routees to spawn.
  /// * `hash_key_mapper` - Function that extracts a hash key from each message.
  ///
  /// # Panics
  ///
  /// Panics if `nr_of_instances` is zero.
  #[must_use]
  pub fn new<F>(nr_of_instances: usize, hash_key_mapper: F) -> Self
  where
    F: Fn(&AnyMessage) -> u64 + Send + Sync + 'static, {
    assert!(nr_of_instances > 0, "nr_of_instances must be positive");
    Self {
      nr_of_instances,
      hash_key_mapper: ArcShared::new(hash_key_mapper),
      hash_key_mapper_kind: ConsistentHashingHashKeyMapperKind::CustomClosure,
      router_dispatcher: String::from("default-dispatcher"),
    }
  }

  /// Creates a wire-safe consistent-hashing pool that uses explicit envelope keys.
  ///
  /// Local routing derives keys from [`ConsistentHashableEnvelope::hash_key`].
  /// Messages that are not wrapped in a consistent-hashable envelope share the
  /// fixed fallback key `0`.
  ///
  /// # Panics
  ///
  /// Panics if `nr_of_instances` is zero.
  #[must_use]
  pub fn new_envelope_hash_key(nr_of_instances: usize) -> Self {
    assert!(nr_of_instances > 0, "nr_of_instances must be positive");
    Self {
      nr_of_instances,
      hash_key_mapper: ArcShared::new(|message: &AnyMessage| {
        message.downcast_ref::<ConsistentHashableEnvelope>().map_or(0, ConsistentHashableEnvelope::hash_key)
      }),
      hash_key_mapper_kind: ConsistentHashingHashKeyMapperKind::EnvelopeHashKey,
      router_dispatcher: String::from("default-dispatcher"),
    }
  }

  /// Overrides the dispatcher used for the router head actor.
  #[must_use]
  pub fn with_dispatcher(mut self, dispatcher: String) -> Self {
    self.router_dispatcher = dispatcher;
    self
  }

  /// Creates the routing logic represented by this pool.
  #[must_use]
  pub(crate) fn create_routing_logic(&self) -> ConsistentHashingRoutingLogic {
    let mapper = self.hash_key_mapper.clone();
    ConsistentHashingRoutingLogic::new(move |msg: &AnyMessage| mapper(msg))
  }

  /// Returns the mapper category used by this pool.
  #[must_use]
  pub(crate) const fn hash_key_mapper_kind(&self) -> ConsistentHashingHashKeyMapperKind {
    self.hash_key_mapper_kind
  }
}

impl RouterConfig for ConsistentHashingPool {
  type Logic = ConsistentHashingRoutingLogic;

  fn create_router(&self) -> Router<Self::Logic> {
    Router::new(self.create_routing_logic(), Vec::new())
  }

  fn router_dispatcher(&self) -> String {
    self.router_dispatcher.clone()
  }
}

impl Pool for ConsistentHashingPool {
  fn nr_of_instances(&self) -> usize {
    self.nr_of_instances
  }
}
