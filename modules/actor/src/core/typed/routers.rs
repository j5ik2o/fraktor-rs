//! Pekko-inspired router factories.

use crate::core::typed::{behavior::Behavior, pool_router_builder::PoolRouterBuilder};

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
}
