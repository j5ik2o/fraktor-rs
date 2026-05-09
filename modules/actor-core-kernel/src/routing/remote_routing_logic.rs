//! Type-erased routing logic used by remote router configurations.

#[cfg(test)]
mod tests;

use crate::{
  actor::messaging::AnyMessage,
  routing::{
    ConsistentHashingRoutingLogic, RandomRoutingLogic, RoundRobinRoutingLogic, Routee, RoutingLogic,
    SmallestMailboxRoutingLogic,
  },
};

/// Routing logic wrapper for pools accepted by `RemoteRouterConfig`.
pub enum RemoteRoutingLogic {
  /// Round-robin routing logic.
  RoundRobin(RoundRobinRoutingLogic),
  /// Smallest-mailbox routing logic.
  SmallestMailbox(SmallestMailboxRoutingLogic),
  /// Random routing logic.
  Random(RandomRoutingLogic),
  /// Consistent-hashing routing logic.
  ConsistentHashing(ConsistentHashingRoutingLogic),
}

impl RoutingLogic for RemoteRoutingLogic {
  fn select<'a>(&self, message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    match self {
      | Self::RoundRobin(logic) => logic.select(message, routees),
      | Self::SmallestMailbox(logic) => logic.select(message, routees),
      | Self::Random(logic) => logic.select(message, routees),
      | Self::ConsistentHashing(logic) => logic.select(message, routees),
    }
  }

  fn select_index(&self, routees: &[Routee]) -> usize {
    match self {
      | Self::RoundRobin(logic) => logic.select_index(routees),
      | Self::SmallestMailbox(logic) => logic.select_index(routees),
      | Self::Random(logic) => logic.select_index(routees),
      | Self::ConsistentHashing(logic) => logic.select_index(routees),
    }
  }
}
