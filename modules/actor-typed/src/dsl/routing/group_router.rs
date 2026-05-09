//! Public group router type for typed routing DSL.

use alloc::{format, string::String, vec, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_actor_core_kernel_rs::event::logging::LogLevel;
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};
use portable_atomic::AtomicU64;

use crate::{
  TypedActorRef,
  behavior::Behavior,
  dsl::Behaviors,
  message_and_signals::BehaviorSignal,
  props::TypedProps,
  receptionist::{Listing, Receptionist, ReceptionistCommand, ServiceKey},
};

#[cfg(test)]
mod tests;

/// Configures a group router behavior.
///
/// Unlike a pool router that spawns its own children, a group router discovers
/// routees dynamically via the Receptionist service. It subscribes to listing
/// changes for the provided [`ServiceKey`] and updates its routee set
/// accordingly.
pub struct GroupRouter<M>
where
  M: Send + Sync + Clone + 'static, {
  service_key: ServiceKey<M>,
  strategy:    GroupRouteStrategy<M>,
}

impl<M> GroupRouter<M>
where
  M: Send + Sync + Clone + 'static,
{
  /// Creates a new group router builder for the given service key.
  ///
  /// Messages are routed using random selection by default.
  #[must_use]
  pub const fn new(service_key: ServiceKey<M>) -> Self {
    Self { service_key, strategy: GroupRouteStrategy::Random { seed: 0 } }
  }

  /// Routes messages by random selection across the available routees.
  #[must_use]
  pub fn with_random_routing(mut self, seed: u64) -> Self {
    self.strategy = GroupRouteStrategy::Random { seed };
    self
  }

  /// Routes messages by round-robin selection across the available routees.
  #[must_use]
  pub fn with_round_robin_routing(mut self) -> Self {
    self.strategy = GroupRouteStrategy::RoundRobin;
    self
  }

  /// Routes messages by rendezvous hashing derived from each message.
  #[must_use]
  pub fn with_consistent_hash_routing<F>(mut self, hash_fn: F) -> Self
  where
    F: Fn(&M) -> String + Send + Sync + 'static, {
    self.strategy = GroupRouteStrategy::ConsistentHash { hash_fn: ArcShared::new(hash_fn) };
    self
  }

  fn into_behavior(self) -> Behavior<M> {
    self.build_with_optional_receptionist(None)
  }

  /// Builds the group router with an explicit receptionist reference override.
  #[must_use]
  pub fn build_with_receptionist(self, receptionist_ref: TypedActorRef<ReceptionistCommand>) -> Behavior<M> {
    self.build_with_optional_receptionist(Some(receptionist_ref))
  }

  fn build_with_optional_receptionist(
    self,
    receptionist_override: Option<TypedActorRef<ReceptionistCommand>>,
  ) -> Behavior<M> {
    let key = self.service_key;
    let strategy = self.strategy;
    let routees = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::<TypedActorRef<M>>::new());
    let routees_for_listing = routees.clone();
    let routees_for_message = routees;

    Behaviors::setup(move |ctx| {
      let key_for_signal = key.clone();
      let Some(receptionist_ref) = receptionist_override.as_ref().cloned().or_else(|| ctx.system().receptionist_ref())
      else {
        return Behaviors::stopped();
      };
      let receptionist = SharedLock::new_with_driver::<DefaultMutex<_>>(receptionist_ref);

      let routees_updater = routees_for_listing.clone();
      let listing_factory = ArcShared::new(move || -> Behavior<Listing> {
        let routees_updater = routees_updater.clone();
        Behaviors::receive_message(move |ctx, listing: &Listing| {
          let typed_refs = match listing.typed_refs::<M>() {
            | Ok(typed_refs) => typed_refs,
            | Err(error) => {
              let message = format!(
                "group router ignored listing update due to type mismatch for service {}: {:?}",
                listing.service_id(),
                error
              );
              ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
              return Ok(Behaviors::same());
            },
          };
          routees_updater.with_lock(|guard| *guard = typed_refs);
          Ok(Behaviors::same())
        })
      });

      let listing_props = TypedProps::<Listing>::from_behavior_factory(move || (*listing_factory)());
      let listing_ref = match ctx.spawn_child(&listing_props) {
        | Ok(child) => child.actor_ref(),
        | Err(_) => return Behaviors::stopped(),
      };

      let subscribe_cmd = Receptionist::subscribe(&key, listing_ref.clone());
      if let Err(error) = receptionist.with_lock(|receptionist| receptionist.try_tell(subscribe_cmd)) {
        ctx.system().emit_log(
          LogLevel::Warn,
          format!("group router failed to subscribe to receptionist: {:?}", error),
          Some(ctx.pid()),
          None,
        );
      }
      let receptionist_for_signal = receptionist;
      let listing_ref_for_signal = listing_ref;

      let strategy_for_message = strategy.clone();
      let round_robin_index = AtomicUsize::new(0);
      let random_seed = AtomicU64::new(match &strategy_for_message {
        | GroupRouteStrategy::Random { seed } => *seed,
        | _ => 0,
      });
      let routees_for_receive = routees_for_message.clone();

      Behaviors::receive_message(move |ctx, message: &M| {
        let targets = {
          let guard = routees_for_receive.with_lock(|routees| routees.clone());
          if guard.is_empty() {
            return Ok(Behaviors::same());
          }
          let idx = match &strategy_for_message {
            | GroupRouteStrategy::RoundRobin => round_robin_index.fetch_add(1, Ordering::Relaxed) % guard.len(),
            | GroupRouteStrategy::Random { seed: _ } => {
              let seed = random_seed.fetch_add(1, Ordering::Relaxed);
              pseudo_random_index(seed, guard.len())
            },
            | GroupRouteStrategy::ConsistentHash { hash_fn } => rendezvous_hash_index(&hash_fn(message), &guard),
          };
          vec![guard[idx].clone()]
        };
        for mut target in targets {
          if let Err(error) = target.try_tell(message.clone()) {
            ctx.system().emit_log(
              LogLevel::Warn,
              format!("group router failed to deliver message to routee: {:?}", error),
              Some(ctx.pid()),
              None,
            );
          }
        }
        Ok(Behaviors::same())
      })
      .receive_signal(move |ctx, signal| {
        if matches!(signal, BehaviorSignal::PostStop) {
          let unsubscribe = Receptionist::unsubscribe(&key_for_signal, listing_ref_for_signal.clone());
          if let Err(error) = receptionist_for_signal.with_lock(|receptionist| receptionist.try_tell(unsubscribe)) {
            ctx.system().emit_log(
              LogLevel::Warn,
              format!("group router failed to unsubscribe from receptionist: {:?}", error),
              Some(ctx.pid()),
              None,
            );
          }
        }
        Ok(Behaviors::same())
      })
    })
  }
}

impl<M> From<GroupRouter<M>> for Behavior<M>
where
  M: Send + Sync + Clone + 'static,
{
  fn from(router: GroupRouter<M>) -> Self {
    router.into_behavior()
  }
}

#[derive(Clone)]
enum GroupRouteStrategy<M>
where
  M: Send + Sync + Clone + 'static, {
  RoundRobin,
  Random { seed: u64 },
  ConsistentHash { hash_fn: ArcShared<dyn Fn(&M) -> String + Send + Sync> },
}

const fn pseudo_random_index(seed: u64, len: usize) -> usize {
  let mixed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
  (mixed as usize) % len
}

pub(super) fn rendezvous_hash_index<M>(value: &str, routees: &[TypedActorRef<M>]) -> usize
where
  M: Send + Sync + Clone + 'static, {
  let key_hash = stable_hash(value.as_bytes());
  routees
    .iter()
    .enumerate()
    .max_by_key(|(_, routee)| rendezvous_score(key_hash, routee.pid().value(), routee.pid().generation()))
    .map(|(idx, _)| idx)
    .unwrap_or(0)
}

fn rendezvous_score(key_hash: u64, pid_value: u64, pid_generation: u32) -> u64 {
  let mut hash = key_hash;
  for byte in pid_value.to_le_bytes() {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(1099511628211);
  }
  for byte in pid_generation.to_le_bytes() {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(1099511628211);
  }
  hash
}

fn stable_hash(bytes: &[u8]) -> u64 {
  let mut hash = 14695981039346656037_u64;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(1099511628211);
  }
  hash
}
