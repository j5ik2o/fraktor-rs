//! Pool router configuration using smallest-mailbox routing logic.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use super::{
  Router, pool::Pool, router_config::RouterConfig, smallest_mailbox_routing_logic::SmallestMailboxRoutingLogic,
};

/// Pool router that selects the routee with the smallest mailbox.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.SmallestMailboxPool`.
///
/// Only applicable to local actors whose mailbox size is observable.
/// When no mailbox metrics are available, falls back to the first routee
/// in the routee slice.
pub struct SmallestMailboxPool {
  nr_of_instances:   usize,
  router_dispatcher: String,
}

impl SmallestMailboxPool {
  /// Creates a new smallest-mailbox pool configuration.
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

impl RouterConfig for SmallestMailboxPool {
  type Logic = SmallestMailboxRoutingLogic;

  fn create_router(&self) -> Router<Self::Logic> {
    Router::new(SmallestMailboxRoutingLogic::new(), Vec::new())
  }

  fn router_dispatcher(&self) -> String {
    self.router_dispatcher.clone()
  }
}

impl Pool for SmallestMailboxPool {
  fn nr_of_instances(&self) -> usize {
    self.nr_of_instances
  }
}
