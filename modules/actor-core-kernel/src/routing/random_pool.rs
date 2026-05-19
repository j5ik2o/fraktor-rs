//! Pool router configuration using random routing logic.

#[cfg(test)]
#[path = "random_pool_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{Router, pool::Pool, random_routing_logic::RandomRoutingLogic, router_config::RouterConfig};

pub(crate) const DEFAULT_RANDOM_POOL_SEED: u64 = 1;

/// Pool router that selects routees pseudo-randomly.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RandomPool`.
pub struct RandomPool {
  nr_of_instances:   usize,
  router_dispatcher: String,
}

impl RandomPool {
  /// Creates a new random pool configuration.
  ///
  /// # Panics
  ///
  /// Panics if `nr_of_instances` is zero.
  #[must_use]
  pub fn new(nr_of_instances: usize) -> Self {
    assert!(nr_of_instances > 0, "nr_of_instances must be positive");
    Self { nr_of_instances, router_dispatcher: String::from("default-dispatcher") }
  }

  /// Overrides the dispatcher used for the router head actor.
  #[must_use]
  pub fn with_dispatcher(mut self, dispatcher: String) -> Self {
    self.router_dispatcher = dispatcher;
    self
  }
}

impl RouterConfig for RandomPool {
  type Logic = RandomRoutingLogic;

  fn create_router(&self) -> Router<Self::Logic> {
    Router::new(RandomRoutingLogic::new(DEFAULT_RANDOM_POOL_SEED), Vec::new())
  }

  fn router_dispatcher(&self) -> String {
    self.router_dispatcher.clone()
  }
}

impl Pool for RandomPool {
  fn nr_of_instances(&self) -> usize {
    self.nr_of_instances
  }
}
