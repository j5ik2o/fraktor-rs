//! Type-erased pool configuration accepted by `RemoteRouterConfig`.

#[cfg(test)]
#[path = "remote_router_pool_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use crate::routing::{
  ConsistentHashingPool, Pool, RandomPool, RandomRoutingLogic, RemoteRoutingLogic, RoundRobinPool,
  RoundRobinRoutingLogic, Router, RouterConfig, SmallestMailboxPool, SmallestMailboxRoutingLogic,
  random_pool::DEFAULT_RANDOM_POOL_SEED,
};

/// Pool wrapper for remote router deployment.
pub enum RemoteRouterPool {
  /// Round-robin pool.
  RoundRobin(RoundRobinPool),
  /// Smallest-mailbox pool.
  SmallestMailbox(SmallestMailboxPool),
  /// Random pool.
  Random(RandomPool),
  /// Consistent-hashing pool.
  ConsistentHashing(ConsistentHashingPool),
}

impl RemoteRouterPool {
  /// Creates a router with the local pool's routing logic.
  #[must_use]
  pub fn create_router(&self) -> Router<RemoteRoutingLogic> {
    match self {
      | Self::RoundRobin(_) => Router::new(RemoteRoutingLogic::RoundRobin(RoundRobinRoutingLogic::new()), Vec::new()),
      | Self::SmallestMailbox(_) => {
        Router::new(RemoteRoutingLogic::SmallestMailbox(SmallestMailboxRoutingLogic::new()), Vec::new())
      },
      | Self::Random(_) => {
        Router::new(RemoteRoutingLogic::Random(RandomRoutingLogic::new(DEFAULT_RANDOM_POOL_SEED)), Vec::new())
      },
      | Self::ConsistentHashing(pool) => {
        Router::new(RemoteRoutingLogic::ConsistentHashing(pool.create_routing_logic()), Vec::new())
      },
    }
  }

  /// Returns the initial number of routee instances.
  #[must_use]
  pub fn nr_of_instances(&self) -> usize {
    match self {
      | Self::RoundRobin(pool) => pool.nr_of_instances(),
      | Self::SmallestMailbox(pool) => pool.nr_of_instances(),
      | Self::Random(pool) => pool.nr_of_instances(),
      | Self::ConsistentHashing(pool) => pool.nr_of_instances(),
    }
  }

  /// Returns the router dispatcher configured by the local pool.
  #[must_use]
  pub fn router_dispatcher(&self) -> String {
    match self {
      | Self::RoundRobin(pool) => pool.router_dispatcher(),
      | Self::SmallestMailbox(pool) => pool.router_dispatcher(),
      | Self::Random(pool) => pool.router_dispatcher(),
      | Self::ConsistentHashing(pool) => pool.router_dispatcher(),
    }
  }

  /// Returns whether the local pool has a dynamic resizer.
  #[must_use]
  pub fn has_resizer(&self) -> bool {
    match self {
      | Self::RoundRobin(pool) => pool.has_resizer(),
      | Self::SmallestMailbox(pool) => pool.has_resizer(),
      | Self::Random(pool) => pool.has_resizer(),
      | Self::ConsistentHashing(pool) => pool.has_resizer(),
    }
  }

  /// Returns whether the local pool uses a dedicated pool dispatcher.
  #[must_use]
  pub fn use_pool_dispatcher(&self) -> bool {
    match self {
      | Self::RoundRobin(pool) => pool.use_pool_dispatcher(),
      | Self::SmallestMailbox(pool) => pool.use_pool_dispatcher(),
      | Self::Random(pool) => pool.use_pool_dispatcher(),
      | Self::ConsistentHashing(pool) => pool.use_pool_dispatcher(),
    }
  }

  /// Returns whether the router should stop when every routee is removed.
  #[must_use]
  pub fn stop_router_when_all_routees_removed(&self) -> bool {
    match self {
      | Self::RoundRobin(pool) => pool.stop_router_when_all_routees_removed(),
      | Self::SmallestMailbox(pool) => pool.stop_router_when_all_routees_removed(),
      | Self::Random(pool) => pool.stop_router_when_all_routees_removed(),
      | Self::ConsistentHashing(pool) => pool.stop_router_when_all_routees_removed(),
    }
  }
}

impl From<RoundRobinPool> for RemoteRouterPool {
  fn from(pool: RoundRobinPool) -> Self {
    Self::RoundRobin(pool)
  }
}

impl From<SmallestMailboxPool> for RemoteRouterPool {
  fn from(pool: SmallestMailboxPool) -> Self {
    Self::SmallestMailbox(pool)
  }
}

impl From<RandomPool> for RemoteRouterPool {
  fn from(pool: RandomPool) -> Self {
    Self::Random(pool)
  }
}

impl From<ConsistentHashingPool> for RemoteRouterPool {
  fn from(pool: ConsistentHashingPool) -> Self {
    Self::ConsistentHashing(pool)
  }
}
