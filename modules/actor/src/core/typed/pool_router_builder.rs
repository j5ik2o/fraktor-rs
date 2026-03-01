//! Builder for configuring and constructing pool routers.

#[cfg(test)]
mod tests;

use alloc::{vec, vec::Vec};
use core::sync::atomic::AtomicUsize;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use portable_atomic::{AtomicU64, Ordering};

use crate::core::{
  event::logging::LogLevel,
  typed::{
    Behaviors, actor::TypedActorRefGeneric, behavior::Behavior, behavior_signal::BehaviorSignal,
    props::TypedPropsGeneric,
  },
};

/// Configures and builds a pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors and distributes
/// incoming messages to them using round-robin routing.
pub struct PoolRouterBuilderGeneric<M, TB = NoStdToolbox>
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static, {
  pool_size:        usize,
  behavior_factory: ArcShared<dyn Fn() -> Behavior<M, TB> + Send + Sync>,
  strategy:         PoolRouteStrategy<M>,
}

/// Type alias for [`PoolRouterBuilderGeneric`] with the default [`NoStdToolbox`].
pub type PoolRouterBuilder<M> = PoolRouterBuilderGeneric<M, NoStdToolbox>;

impl<M, TB> PoolRouterBuilderGeneric<M, TB>
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new pool router builder with the given factory.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  pub(crate) fn new<F>(pool_size: usize, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<M, TB> + Send + Sync + 'static, {
    assert!(pool_size > 0, "pool size must be positive");
    Self { pool_size, behavior_factory: ArcShared::new(behavior_factory), strategy: PoolRouteStrategy::RoundRobin }
  }

  /// Overrides the pool size.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub const fn with_pool_size(mut self, pool_size: usize) -> Self {
    assert!(pool_size > 0, "pool size must be positive");
    self.pool_size = pool_size;
    self
  }

  /// Routes each incoming message to all routees.
  #[must_use]
  pub fn with_broadcast(mut self) -> Self {
    self.strategy = PoolRouteStrategy::Broadcast;
    self
  }

  /// Routes incoming messages pseudo-randomly across routees.
  #[must_use]
  pub fn with_random(mut self, seed: u64) -> Self {
    self.strategy = PoolRouteStrategy::Random { seed };
    self
  }

  /// Routes incoming messages by a stable hash function.
  #[must_use]
  pub fn with_consistent_hash<F>(mut self, hash_fn: F) -> Self
  where
    F: Fn(&M) -> u64 + Send + Sync + 'static, {
    self.strategy = PoolRouteStrategy::ConsistentHash { hash_fn: ArcShared::new(hash_fn) };
    self
  }

  /// Routes incoming messages to the routee with the smallest mailbox size.
  #[must_use]
  pub fn with_smallest_mailbox(mut self) -> Self {
    self.strategy = PoolRouteStrategy::SmallestMailbox;
    self
  }

  /// Builds the pool router as a [`Behavior`].
  #[must_use]
  #[allow(clippy::redundant_closure)]
  pub fn build(self) -> Behavior<M, TB> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;
    let strategy = self.strategy;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedPropsGeneric::<M, TB>::from_behavior_factory(move || bf());

      let mut routee_vec: Vec<TypedActorRefGeneric<M, TB>> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => routee_vec.push(child.actor_ref().clone()),
          | Err(e) => {
            let msg = alloc::format!("pool router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()));
            break;
          },
        }
      }

      let routee_count = routee_vec.len();
      let mutex = <TB::MutexFamily as SyncMutexFamily>::create(routee_vec);
      let routees: ArcShared<ToolboxMutex<Vec<TypedActorRefGeneric<M, TB>>, TB>> = ArcShared::new(mutex);
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;
      let index = AtomicUsize::new(0);
      let random_seed = AtomicU64::new(0);
      let dispatch_counts = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(vec![0_usize; routee_count]));
      let strategy_for_msg = strategy.clone();

      Behaviors::receive_message(move |_ctx, message: &M| {
        let mut targets: Vec<TypedActorRefGeneric<M, TB>> = Vec::new();
        {
          let guard = routees_for_msg.lock();
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          match &strategy_for_msg {
            | PoolRouteStrategy::RoundRobin => {
              let idx = index.fetch_add(1, Ordering::Relaxed) % guard.len();
              targets.push(guard[idx].clone());
            },
            | PoolRouteStrategy::Broadcast => {
              targets.extend(guard.iter().cloned());
            },
            | PoolRouteStrategy::Random { seed } => {
              let mixed_seed = random_seed.fetch_add(1, Ordering::Relaxed) ^ *seed;
              let idx = pseudo_random_index(mixed_seed, guard.len());
              targets.push(guard[idx].clone());
            },
            | PoolRouteStrategy::ConsistentHash { hash_fn } => {
              let idx = (hash_fn(message) as usize) % guard.len();
              targets.push(guard[idx].clone());
            },
            | PoolRouteStrategy::SmallestMailbox => {
              let idx = select_smallest_mailbox_index(&guard, &dispatch_counts);
              targets.push(guard[idx].clone());
            },
          }
        }
        for mut target in targets {
          let _ = target.tell(message.clone());
        }
        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(pid) => {
          let mut guard = routees_for_sig.lock();
          guard.retain(|r| r.pid() != *pid);
          if guard.is_empty() {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}

#[derive(Clone)]
enum PoolRouteStrategy<M>
where
  M: Send + Sync + Clone + 'static, {
  RoundRobin,
  Broadcast,
  Random { seed: u64 },
  ConsistentHash { hash_fn: ArcShared<dyn Fn(&M) -> u64 + Send + Sync> },
  SmallestMailbox,
}

const fn pseudo_random_index(seed: u64, len: usize) -> usize {
  let mixed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
  (mixed as usize) % len
}

fn select_smallest_mailbox_index<M, TB>(
  routees: &[TypedActorRefGeneric<M, TB>],
  dispatch_counts: &ArcShared<ToolboxMutex<Vec<usize>, TB>>,
) -> usize
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static, {
  let routee_count = routees.len();
  let mut best_index = 0_usize;
  let mut best_len = usize::MAX;
  for (index, routee) in routees.iter().enumerate() {
    let mailbox_len = routee
      .as_untyped()
      .system_state()
      .and_then(|system| system.cell(&routee.pid()))
      .map_or(usize::MAX, |cell| cell.mailbox().user_len());
    if mailbox_len < best_len {
      best_len = mailbox_len;
      best_index = index;
    }
  }

  if best_len == usize::MAX {
    let mut counts = dispatch_counts.lock();
    let mut selected = 0_usize;
    let mut selected_count = usize::MAX;
    for (index, count) in counts.iter().enumerate().take(routee_count) {
      if *count < selected_count {
        selected = index;
        selected_count = *count;
      }
    }
    if let Some(entry) = counts.get_mut(selected) {
      *entry = entry.saturating_add(1);
    }
    return selected;
  }

  best_index
}
