//! Builder for configuring and constructing pool routers.

#[cfg(test)]
mod tests;

use alloc::{vec, vec::Vec};
use core::sync::atomic::AtomicUsize;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
use portable_atomic::{AtomicU64, Ordering};

use crate::core::{
  event::logging::LogLevel,
  typed::{
    Behaviors, actor::TypedActorRef, behavior::Behavior, behavior_signal::BehaviorSignal, props::TypedProps,
    resizer::Resizer,
  },
};

type RouteSelector<M> = dyn Fn(&[TypedActorRef<M>], &M) -> Vec<TypedActorRef<M>> + Send + Sync;
type BroadcastPredicate<M> = dyn Fn(&M) -> bool + Send + Sync;

/// Configures and builds a pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors and distributes
/// incoming messages to them using round-robin routing.
pub struct PoolRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static, {
  pool_size:           usize,
  behavior_factory:    ArcShared<dyn Fn() -> Behavior<M> + Send + Sync>,
  strategy:            PoolRouteStrategy<M>,
  broadcast_predicate: Option<ArcShared<BroadcastPredicate<M>>>,
  resizer:             Option<ArcShared<dyn Resizer>>,
}

impl<M> PoolRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static,
{
  /// Creates a new pool router builder with the given factory.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  pub(crate) fn new<F>(pool_size: usize, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    assert!(pool_size > 0, "pool size must be positive");
    Self {
      pool_size,
      behavior_factory: ArcShared::new(behavior_factory),
      strategy: PoolRouteStrategy::RoundRobin,
      broadcast_predicate: None,
      resizer: None,
    }
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

  /// Routes incoming messages through round-robin selection.
  #[must_use]
  pub fn with_round_robin(mut self) -> Self {
    self.strategy = PoolRouteStrategy::RoundRobin;
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

  /// Broadcasts only messages that satisfy `predicate`.
  #[must_use]
  pub fn with_broadcast_predicate<F>(mut self, predicate: F) -> Self
  where
    F: Fn(&M) -> bool + Send + Sync + 'static, {
    self.broadcast_predicate = Some(ArcShared::new(predicate));
    self
  }

  /// Routes incoming messages to the routee with the smallest mailbox size.
  #[must_use]
  pub fn with_smallest_mailbox(mut self) -> Self {
    self.strategy = PoolRouteStrategy::SmallestMailbox;
    self
  }

  /// Attaches a resizer that dynamically adjusts the pool size.
  ///
  /// The resizer is consulted on each message to decide whether to add or
  /// remove routees. When no resizer is set (the default), the pool size
  /// remains fixed.
  #[must_use]
  pub fn with_resizer<R: Resizer + 'static>(mut self, resizer: R) -> Self {
    self.resizer = Some(ArcShared::new(resizer));
    self
  }

  /// Builds the pool router as a [`Behavior`].
  #[must_use]
  pub fn build(self) -> Behavior<M> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;
    let strategy = self.strategy;
    let broadcast_predicate = self.broadcast_predicate;
    let resizer = self.resizer;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedProps::<M>::from_behavior_factory(move || {
        let factory: &(dyn Fn() -> Behavior<M> + Send + Sync) = &*bf;
        factory()
      });

      let mut routee_vec: Vec<TypedActorRef<M>> = Vec::with_capacity(pool_size);
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

      // Clone props for potential resize spawning.
      let props_for_resize = resizer.as_ref().map(|_| props.clone());

      let routee_count = routee_vec.len();
      let routees = ArcShared::new(RuntimeMutex::new(routee_vec));
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;

      let mut dispatch_counts_for_sig: Option<ArcShared<RuntimeMutex<Vec<usize>>>> = None;
      let mut dispatch_counts_for_msg: Option<ArcShared<RuntimeMutex<Vec<usize>>>> = None;

      let select_targets: ArcShared<RouteSelector<M>> = match strategy.clone() {
        | PoolRouteStrategy::RoundRobin => {
          let index = AtomicUsize::new(0);
          ArcShared::new(move |guard: &[TypedActorRef<M>], _message: &M| {
            let idx = index.fetch_add(1, Ordering::Relaxed) % guard.len();
            vec![guard[idx].clone()]
          })
        },
        | PoolRouteStrategy::Broadcast => ArcShared::new(|guard: &[TypedActorRef<M>], _message: &M| guard.to_vec()),
        | PoolRouteStrategy::Random { seed } => {
          let random_seed = AtomicU64::new(0);
          ArcShared::new(move |guard: &[TypedActorRef<M>], _message: &M| {
            let mixed_seed = random_seed.fetch_add(1, Ordering::Relaxed) ^ seed;
            let idx = pseudo_random_index(mixed_seed, guard.len());
            vec![guard[idx].clone()]
          })
        },
        | PoolRouteStrategy::ConsistentHash { hash_fn } => {
          ArcShared::new(move |guard: &[TypedActorRef<M>], message: &M| {
            let idx = (hash_fn(message) as usize) % guard.len();
            vec![guard[idx].clone()]
          })
        },
        | PoolRouteStrategy::SmallestMailbox => {
          let dispatch_counts = ArcShared::new(RuntimeMutex::new(vec![0_usize; routee_count]));
          dispatch_counts_for_sig = Some(dispatch_counts.clone());
          dispatch_counts_for_msg = Some(dispatch_counts.clone());
          ArcShared::new(move |guard: &[TypedActorRef<M>], _message: &M| {
            let idx = select_smallest_mailbox_index(guard, &dispatch_counts);
            vec![guard[idx].clone()]
          })
        },
      };

      let broadcast_predicate_for_message = broadcast_predicate.clone();
      let resizer_for_msg = resizer.clone();
      let dispatch_counts_for_resize = dispatch_counts_for_msg;
      let message_counter = AtomicU64::new(0);
      Behaviors::receive_message(move |ctx, message: &M| {
        // --- Resize check ---
        if let Some(ref resizer) = resizer_for_msg {
          let counter = message_counter.fetch_add(1, Ordering::Relaxed);
          if resizer.is_time_for_resize(counter) {
            let current_count = routees_for_msg.lock().len();
            let delta = resizer.resize(current_count);
            if delta > 0 {
              if let Some(ref resize_props) = props_for_resize {
                let mut new_routees = Vec::new();
                for _ in 0..delta {
                  match ctx.spawn_child_watched(resize_props) {
                    | Ok(child) => new_routees.push(child.actor_ref().clone()),
                    | Err(e) => {
                      let msg = alloc::format!("pool router resize failed to spawn child: {:?}", e);
                      ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()));
                      break;
                    },
                  }
                }
                if !new_routees.is_empty() {
                  // routee 配列と dispatch_counts を同一 routees guard 内で更新する。
                  // select_targets も routees guard を取得してから dispatch_counts を参照するため、
                  // この guard を保持している間は select_targets が割り込めず不整合は発生しない。
                  let mut guard = routees_for_msg.lock();
                  guard.extend(new_routees);
                  if let Some(ref dc) = dispatch_counts_for_resize {
                    let mut counts = dc.lock();
                    counts.resize(guard.len(), 0);
                  }
                }
              }
            } else if delta < 0 {
              let abs_delta = (-delta) as usize;
              let to_stop: Vec<TypedActorRef<M>> = {
                let mut guard = routees_for_msg.lock();
                // routee を全て削除しない — 最低1台は残す
                let remove_count = abs_delta.min(guard.len().saturating_sub(1));
                let split_at = guard.len() - remove_count;
                let removed = guard.split_off(split_at);
                // SmallestMailbox 用 dispatch_counts も routees guard 内で同期して縮小する
                if let Some(ref dc) = dispatch_counts_for_resize {
                  let mut counts = dc.lock();
                  counts.truncate(guard.len());
                }
                removed
              };
              for routee in &to_stop {
                if let Err(e) = ctx.stop_actor_by_ref(routee) {
                  ctx.system().emit_log(
                    LogLevel::Warn,
                    alloc::format!("pool router failed to stop routee during resize: {:?}", e),
                    Some(ctx.pid()),
                  );
                }
              }
            }
          }
        }

        // --- Route message ---
        let targets = {
          let guard = routees_for_msg.lock();
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          if let Some(predicate) = broadcast_predicate_for_message.as_ref() {
            if predicate(message) { guard.to_vec() } else { (select_targets)(&guard, message) }
          } else {
            (select_targets)(&guard, message)
          }
        };
        for target in targets {
          if let Err(e) = target.try_tell(message.clone()) {
            ctx.system().emit_log(
              LogLevel::Warn,
              alloc::format!("pool router failed to send message to routee: {:?}", e),
              Some(ctx.pid()),
            );
          }
        }
        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(pid) => {
          let mut guard = routees_for_sig.lock();
          if let Some(pos) = guard.iter().position(|r| r.pid() == *pid) {
            guard.remove(pos);
            if let Some(ref dc) = dispatch_counts_for_sig {
              let mut counts = dc.lock();
              if pos < counts.len() {
                counts.remove(pos);
              }
            }
          }
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

fn select_smallest_mailbox_index<M>(
  routees: &[TypedActorRef<M>],
  dispatch_counts: &ArcShared<RuntimeMutex<Vec<usize>>>,
) -> usize
where
  M: Send + Sync + Clone + 'static, {
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
