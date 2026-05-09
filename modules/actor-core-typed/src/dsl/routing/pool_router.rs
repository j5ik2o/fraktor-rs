//! Public pool router type for typed routing DSL.

use alloc::{vec, vec::Vec};
use core::sync::atomic::AtomicUsize;

use fraktor_actor_core_kernel_rs::{
  event::logging::LogLevel,
  routing::{FNV_OFFSET_BASIS, Routee, RoutingLogic, SmallestMailboxRoutingLogic, mix_hash, rendezvous_score},
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};
use portable_atomic::{AtomicU64, Ordering};

use super::resizer::Resizer;
use crate::{
  TypedActorRef, behavior::Behavior, dsl::Behaviors, message_and_signals::BehaviorSignal, props::TypedProps,
};

#[cfg(test)]
mod tests;

type RouteSelector<M> = dyn Fn(&[TypedActorRef<M>], &M) -> Vec<TypedActorRef<M>> + Send + Sync;
type BroadcastPredicate<M> = dyn Fn(&M) -> bool + Send + Sync;
type RouteePropsMapper<M> = dyn Fn(TypedProps<M>) -> TypedProps<M> + Send + Sync;

/// Configures a pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors and distributes
/// incoming messages to them using round-robin routing by default.
pub struct PoolRouter<M>
where
  M: Send + Sync + Clone + 'static, {
  pool_size:           usize,
  behavior_factory:    ArcShared<dyn Fn() -> Behavior<M> + Send + Sync>,
  strategy:            PoolRouteStrategy<M>,
  broadcast_predicate: Option<ArcShared<BroadcastPredicate<M>>>,
  resizer:             Option<ArcShared<dyn Resizer>>,
  routee_props_mapper: Option<ArcShared<RouteePropsMapper<M>>>,
}

impl<M> PoolRouter<M>
where
  M: Send + Sync + Clone + 'static,
{
  /// Creates a new pool router builder with the given factory.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub fn new<F>(pool_size: usize, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    assert!(pool_size > 0, "pool size must be positive");
    Self {
      pool_size,
      behavior_factory: ArcShared::new(behavior_factory),
      strategy: PoolRouteStrategy::RoundRobin,
      broadcast_predicate: None,
      resizer: None,
      routee_props_mapper: None,
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

  /// Applies a transformation to the [`TypedProps`] used when spawning each routee.
  ///
  /// The `props_mapper` receives the default props built from the behavior factory
  /// and returns modified props. This allows adding tags, adjusting dispatcher
  /// settings, or any other props customization without replacing the entire props.
  ///
  /// Corresponds to Pekko's `PoolRouter.withRouteeProps`.
  #[must_use]
  pub fn with_routee_props(
    mut self,
    props_mapper: impl Fn(TypedProps<M>) -> TypedProps<M> + Send + Sync + 'static,
  ) -> Self {
    self.routee_props_mapper = Some(ArcShared::new(props_mapper));
    self
  }

  fn into_behavior(self) -> Behavior<M> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;
    let strategy = self.strategy;
    let broadcast_predicate = self.broadcast_predicate;
    let resizer = self.resizer;
    let routee_props_mapper = self.routee_props_mapper;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedProps::<M>::from_behavior_factory(move || {
        let factory: &(dyn Fn() -> Behavior<M> + Send + Sync) = &*bf;
        factory()
      });
      let props = if let Some(ref mapper) = routee_props_mapper { mapper(props) } else { props };

      let mut routee_vec: Vec<TypedActorRef<M>> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => routee_vec.push(child.into_actor_ref()),
          | Err(e) => {
            let msg = alloc::format!("pool router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
            break;
          },
        }
      }

      let props_for_resize = resizer.as_ref().map(|_| props.clone());

      let routees = SharedLock::new_with_driver::<DefaultMutex<_>>(routee_vec);
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;

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
            let idx = select_consistent_hash_index(guard, message, &*hash_fn);
            vec![guard[idx].clone()]
          })
        },
        | PoolRouteStrategy::SmallestMailbox => ArcShared::new(move |guard: &[TypedActorRef<M>], _message: &M| {
          let idx = select_smallest_mailbox_index(guard);
          vec![guard[idx].clone()]
        }),
      };

      let broadcast_predicate_for_message = broadcast_predicate.clone();
      let resizer_for_msg = resizer.clone();
      let message_counter = AtomicU64::new(0);
      Behaviors::receive_message(move |ctx, message: &M| {
        if let Some(ref resizer) = resizer_for_msg {
          let counter = message_counter.fetch_add(1, Ordering::Relaxed);
          // Pekko `ResizablePoolCell` 相当の順序で呼び出す:
          // 1. `is_time_for_resize` を先にチェック（軽量、通常 false）
          // 2. true の場合のみ mailbox スナップショットを取り、`report_message_count` → `resize` を
          //    **同じスナップショット** で実行する。
          //
          // `report_message_count` は内部で `check_time` を更新するため、先に
          // 呼んでしまうと `is_time_for_resize` の時刻差判定が常にゼロとなり
          // resize が発火しなくなる（Pekko も同様の順序制約を持つ。
          // 参照: `OptimalSizeExploringResizer.scala:201-203, 262` および
          // `Resizer.scala:286-309`）。
          if resizer.is_time_for_resize(counter) {
            let mailbox_sizes = routees_for_msg.with_lock(|routees| observe_routee_mailbox_sizes(routees.as_slice()));
            resizer.report_message_count(&mailbox_sizes, counter);
            let delta = resizer.resize(&mailbox_sizes);
            if delta > 0 {
              if let Some(ref resize_props) = props_for_resize {
                let mut new_routees = Vec::new();
                for _ in 0..delta {
                  match ctx.spawn_child_watched(resize_props) {
                    | Ok(child) => new_routees.push(child.into_actor_ref()),
                    | Err(e) => {
                      let msg = alloc::format!("pool router resize failed to spawn child: {:?}", e);
                      ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
                      break;
                    },
                  }
                }
                if !new_routees.is_empty() {
                  routees_for_msg.with_lock(|guard| {
                    guard.extend(new_routees);
                  });
                }
              }
            } else if delta < 0 {
              let abs_delta = (-delta) as usize;
              // Pekko 原典 (`Resizer.scala:305` の `currentRoutees.drop(...)`) と同じく
              // 末尾の routee を停止対象にする。
              let to_stop: Vec<TypedActorRef<M>> = {
                routees_for_msg.with_lock(|guard| {
                  let remove_count = abs_delta.min(guard.len().saturating_sub(1));
                  let split_at = guard.len() - remove_count;
                  guard.split_off(split_at)
                })
              };
              for routee in &to_stop {
                if let Err(e) = ctx.stop_actor_by_ref(routee) {
                  ctx.system().emit_log(
                    LogLevel::Warn,
                    alloc::format!("pool router failed to stop routee during resize: {:?}", e),
                    Some(ctx.pid()),
                    None,
                  );
                }
              }
            }
          }
        }

        let targets = {
          let guard = routees_for_msg.with_lock(|routees| routees.clone());
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          if let Some(predicate) = broadcast_predicate_for_message.as_ref() {
            if predicate(message) { guard.to_vec() } else { select_targets(&guard, message) }
          } else {
            select_targets(&guard, message)
          }
        };
        for mut target in targets {
          if let Err(error) = target.try_tell(message.clone()) {
            ctx.system().emit_log(
              LogLevel::Warn,
              alloc::format!("pool router failed to deliver message to routee: {:?}", error),
              Some(ctx.pid()),
              None,
            );
          }
        }
        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(terminated) => {
          let pid = terminated.pid();
          let is_empty = routees_for_sig.with_lock(|guard| {
            if let Some(pos) = guard.iter().position(|r| r.pid() == pid) {
              guard.remove(pos);
            }
            guard.is_empty()
          });
          if is_empty {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}

impl<M> From<PoolRouter<M>> for Behavior<M>
where
  M: Send + Sync + Clone + 'static,
{
  fn from(router: PoolRouter<M>) -> Self {
    router.into_behavior()
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

pub(super) const fn pseudo_random_index(seed: u64, len: usize) -> usize {
  let mixed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
  (mixed as usize) % len
}

pub(super) fn select_consistent_hash_index<M>(
  routees: &[TypedActorRef<M>],
  message: &M,
  hash_fn: &(dyn Fn(&M) -> u64 + Send + Sync),
) -> usize
where
  M: Send + Sync + Clone + 'static, {
  assert!(!routees.is_empty(), "routees must not be empty");
  let key_hash = hash_fn(message);
  // kernel 側 routee_identity_hash の ActorRef タグ ([0]) と一致させるためのシード
  let actor_ref_seed = mix_hash(FNV_OFFSET_BASIS, &[0]);
  routees
    .iter()
    .enumerate()
    .max_by_key(|(_, routee)| {
      let pid = routee.pid();
      let hash = mix_hash(actor_ref_seed, &pid.value().to_le_bytes());
      let routee_hash = mix_hash(hash, &pid.generation().to_le_bytes());
      rendezvous_score(key_hash, routee_hash)
    })
    .map(|(index, _)| index)
    .unwrap_or(0)
}

/// Observes current mailbox pending counts for each routee.
///
/// Returns a `Vec<usize>` of the same length as `routees`, where element `i`
/// is the number of pending user messages in routee `i`'s mailbox. When the
/// underlying system or cell cannot be resolved (e.g., the actor has
/// terminated), the entry is `0` — treating unreachable routees as empty
/// matches Pekko's `OptimalSizeExploringResizer` contract, which reasons
/// over observable mailbox pressure.
pub(super) fn observe_routee_mailbox_sizes<M>(routees: &[TypedActorRef<M>]) -> Vec<usize>
where
  M: Send + Sync + Clone + 'static, {
  routees
    .iter()
    .map(|routee| {
      let actor_ref = routee.as_untyped();
      let Some(system) = actor_ref.system_state() else {
        return 0;
      };
      let Some(cell) = system.cell(&actor_ref.pid()) else {
        return 0;
      };
      cell.mailbox().user_len()
    })
    .collect()
}

/// Selects the smallest-mailbox routee index.
///
/// # Panics
///
/// Panics if `routees` is empty. Callers must guard against empty routees
/// (pool_router's message handler does this at the call site).
pub(super) fn select_smallest_mailbox_index<M>(routees: &[TypedActorRef<M>]) -> usize
where
  M: Send + Sync + Clone + 'static, {
  assert!(!routees.is_empty(), "select_smallest_mailbox_index requires non-empty routees");
  // kernel `SmallestMailboxRoutingLogic` に Pekko 互換のスコアリング判定を委譲する。
  // `select_index` は `AnyMessage` の dummy を受け取らず、usize を直接返すため
  // 従来の `AnyMessage::new(())` と pid position 探索を排除できる。
  let untyped_routees: Vec<Routee> = routees.iter().map(|r| Routee::ActorRef(r.as_untyped().clone())).collect();
  let logic = SmallestMailboxRoutingLogic::new();
  logic.select_index(&untyped_routees)
}
