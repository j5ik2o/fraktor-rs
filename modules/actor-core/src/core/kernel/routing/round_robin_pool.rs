//! Pool router configuration using round-robin routing logic.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{Router, pool::Pool, round_robin_routing_logic::RoundRobinRoutingLogic, router_config::RouterConfig};

/// Pool router that selects routees in round-robin order.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RoundRobinPool`.
pub struct RoundRobinPool {
  nr_of_instances:   usize,
  router_dispatcher: String,
}

impl RoundRobinPool {
  /// Creates a new round-robin pool configuration.
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

impl RouterConfig for RoundRobinPool {
  type Logic = RoundRobinRoutingLogic;

  fn create_router(&self) -> Router<Self::Logic> {
    Router::new(RoundRobinRoutingLogic::new(), Vec::new())
  }

  fn router_dispatcher(&self) -> String {
    self.router_dispatcher.clone()
  }
}

impl Pool for RoundRobinPool {
  fn nr_of_instances(&self) -> usize {
    self.nr_of_instances
  }
}
