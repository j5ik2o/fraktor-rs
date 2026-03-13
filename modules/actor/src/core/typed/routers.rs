//! Pekko-inspired router factories.

#[cfg(test)]
mod tests;

use crate::core::typed::{
  behavior::Behavior, group_router_builder::GroupRouterBuilder, pool_router_builder::PoolRouterBuilder,
  service_key::ServiceKey,
};

/// Provides factory methods for creating routers.
pub struct Routers;

impl Routers {
  /// Creates a pool router that spawns `pool_size` child actors using the given factory.
  ///
  /// Messages are distributed using round-robin by default.
  #[must_use]
  pub fn pool<M, F>(pool_size: usize, behavior_factory: F) -> PoolRouterBuilder<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    PoolRouterBuilder::new(pool_size, behavior_factory)
  }

  /// Creates a group router that discovers routees via the Receptionist.
  ///
  /// The router subscribes to listing changes for the provided [`ServiceKey`]
  /// and routes messages to discovered actors using random selection by
  /// default.
  #[must_use]
  pub const fn group<M>(key: ServiceKey<M>) -> GroupRouterBuilder<M>
  where
    M: Send + Sync + Clone + 'static, {
    GroupRouterBuilder::new(key)
  }
}
