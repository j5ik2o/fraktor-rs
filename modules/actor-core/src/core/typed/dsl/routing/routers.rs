//! Pekko-inspired router factories.

#[cfg(test)]
mod tests;

use core::time::Duration;

use super::{
  balancing_pool_router_builder::BalancingPoolRouterBuilder, group_router::GroupRouter, pool_router::PoolRouter,
  scatter_gather_first_completed_router_builder::ScatterGatherFirstCompletedRouterBuilder,
  tail_chopping_router_builder::TailChoppingRouterBuilder,
};
use crate::core::typed::{TypedActorRef, behavior::Behavior, receptionist::ServiceKey};

/// Provides factory methods for creating routers.
pub struct Routers;

impl Routers {
  /// Creates a pool router that spawns `pool_size` child actors using the given factory.
  ///
  /// Messages are distributed using round-robin by default.
  #[must_use]
  pub fn pool<M, F>(pool_size: usize, behavior_factory: F) -> PoolRouter<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    PoolRouter::new(pool_size, behavior_factory)
  }

  /// Creates a group router that discovers routees via the Receptionist.
  ///
  /// The router subscribes to listing changes for the provided [`ServiceKey`]
  /// and routes messages to discovered actors using random selection by
  /// default.
  #[must_use]
  pub const fn group<M>(key: ServiceKey<M>) -> GroupRouter<M>
  where
    M: Send + Sync + Clone + 'static, {
    GroupRouter::new(key)
  }

  /// Creates a scatter-gather-first-completed pool router.
  ///
  /// The router spawns `pool_size` child actors. For each incoming request the
  /// router sends it to **all** routees and returns the first reply. If no
  /// reply arrives within `within`, `timeout_reply` is returned instead.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub fn scatter_gather_first_completed_pool<M, R, BF, CF, EF>(
    pool_size: usize,
    behavior_factory: BF,
    within: Duration,
    create_request: CF,
    extract_reply_to: EF,
    timeout_reply: R,
  ) -> ScatterGatherFirstCompletedRouterBuilder<M, R>
  where
    M: Send + Sync + Clone + 'static,
    R: Send + Sync + Clone + 'static,
    BF: Fn() -> Behavior<M> + Send + Sync + 'static,
    CF: Fn(&M, TypedActorRef<R>) -> M + Send + Sync + 'static,
    EF: Fn(&M) -> Option<TypedActorRef<R>> + Send + Sync + 'static, {
    ScatterGatherFirstCompletedRouterBuilder::new(
      pool_size,
      behavior_factory,
      within,
      create_request,
      extract_reply_to,
      timeout_reply,
    )
  }

  /// Creates a balancing pool router that distributes work via a shared queue.
  ///
  /// All routees pull work from a single shared queue, ensuring that idle
  /// routees receive work first. This mirrors Pekko's `BalancingPool`
  /// semantics. Resizer is intentionally not supported.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub fn balancing_pool<M, F>(pool_size: usize, behavior_factory: F) -> BalancingPoolRouterBuilder<M>
  where
    M: Send + Sync + Clone + 'static,
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    BalancingPoolRouterBuilder::new(pool_size, behavior_factory)
  }

  /// Creates a tail-chopping pool router.
  ///
  /// The router spawns `pool_size` child actors. For each incoming request the
  /// router sends it to routees one at a time, waiting `interval` between each
  /// attempt. The first reply from any routee is returned. If no reply arrives
  /// within `within`, `timeout_reply` is returned instead.
  ///
  /// This pattern reduces tail latency by sending backup requests when a
  /// routee is slow to respond.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub fn tail_chopping_pool<M, R, BF, CF, EF>(
    pool_size: usize,
    behavior_factory: BF,
    within: Duration,
    interval: Duration,
    create_request: CF,
    extract_reply_to: EF,
    timeout_reply: R,
  ) -> TailChoppingRouterBuilder<M, R>
  where
    M: Send + Sync + Clone + 'static,
    R: Send + Sync + Clone + 'static,
    BF: Fn() -> Behavior<M> + Send + Sync + 'static,
    CF: Fn(&M, TypedActorRef<R>) -> M + Send + Sync + 'static,
    EF: Fn(&M) -> Option<TypedActorRef<R>> + Send + Sync + 'static, {
    TailChoppingRouterBuilder::new(
      pool_size,
      behavior_factory,
      within,
      interval,
      create_request,
      extract_reply_to,
      timeout_reply,
    )
  }
}
