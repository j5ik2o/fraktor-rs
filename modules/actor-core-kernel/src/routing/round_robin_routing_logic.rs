//! Round-robin routing logic.

#[cfg(test)]
#[path = "round_robin_routing_logic_test.rs"]
mod tests;

use core::sync::atomic::{AtomicUsize, Ordering};

use super::{routee::Routee, routing_logic::RoutingLogic};
use crate::actor::messaging::AnyMessage;

/// Selects routees in a cyclic round-robin order.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RoundRobinRoutingLogic`.
///
/// Uses an atomic counter so that `select` can be called via `&self`
/// concurrently from multiple threads.
pub struct RoundRobinRoutingLogic {
  counter: AtomicUsize,
}

impl RoundRobinRoutingLogic {
  /// Creates a new round-robin logic with its counter starting at zero.
  #[must_use]
  pub const fn new() -> Self {
    Self::with_initial_counter(0)
  }

  /// Creates a new round-robin logic with the provided initial counter value.
  #[must_use]
  pub(crate) const fn with_initial_counter(counter: usize) -> Self {
    Self { counter: AtomicUsize::new(counter) }
  }
}

impl Default for RoundRobinRoutingLogic {
  fn default() -> Self {
    Self::new()
  }
}

impl RoutingLogic for RoundRobinRoutingLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    if routees.is_empty() {
      static NO_ROUTEE: Routee = Routee::NoRoutee;
      return &NO_ROUTEE;
    }
    let idx = self.counter.fetch_add(1, Ordering::Relaxed) % routees.len();
    &routees[idx]
  }
}
