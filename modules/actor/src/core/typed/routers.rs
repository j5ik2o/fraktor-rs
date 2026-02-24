//! Pekko-inspired router factories.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::typed::{behavior::Behavior, pool_router_builder::PoolRouterBuilderGeneric};

/// Provides factory methods for creating routers.
pub struct Routers;

impl Routers {
  /// Creates a pool router that spawns `pool_size` child actors using the given factory.
  ///
  /// Messages are distributed using round-robin by default.
  #[must_use]
  pub fn pool<M, TB, F>(pool_size: usize, behavior_factory: F) -> PoolRouterBuilderGeneric<M, TB>
  where
    M: Send + Sync + Clone + 'static,
    TB: RuntimeToolbox + 'static,
    F: Fn() -> Behavior<M, TB> + Send + Sync + 'static, {
    PoolRouterBuilderGeneric::new(pool_size, behavior_factory)
  }
}
