//! Consistent-hashing routing logic.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{routee::Routee, routing_logic::RoutingLogic};
use crate::core::kernel::actor::messaging::AnyMessage;

// メッセージからハッシュキーを抽出するマッパー型。
// ConsistentHashingPool::create_router 経由で使用される。
type HashKeyMapper = dyn Fn(&AnyMessage) -> u64 + Send + Sync;

pub(crate) const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
pub(crate) const FNV_PRIME: u64 = 1099511628211;

/// Selects a routee via rendezvous hashing derived from each message.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.ConsistentHashingRoutingLogic`.
///
/// The implementation is stateless and therefore safe to call via `&self`
/// from multiple threads concurrently.
pub struct ConsistentHashingRoutingLogic {
  hash_key_mapper: ArcShared<HashKeyMapper>,
}

impl ConsistentHashingRoutingLogic {
  /// Creates a new consistent-hashing routing logic.
  #[must_use]
  pub fn new<F>(hash_key_mapper: F) -> Self
  where
    F: Fn(&AnyMessage) -> u64 + Send + Sync + 'static, {
    Self { hash_key_mapper: ArcShared::new(hash_key_mapper) }
  }
}

impl RoutingLogic for ConsistentHashingRoutingLogic {
  fn select<'a>(&self, message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    static NO_ROUTEE: Routee = Routee::NoRoutee;
    if routees.is_empty() {
      return &NO_ROUTEE;
    }

    let key_hash = (self.hash_key_mapper)(message);
    let Some((selected_index, _)) =
      routees.iter().enumerate().max_by_key(|(_, routee)| rendezvous_score(key_hash, routee_identity_hash(routee)))
    else {
      return &NO_ROUTEE;
    };

    &routees[selected_index]
  }
}

pub(crate) fn rendezvous_score(key_hash: u64, routee_hash: u64) -> u64 {
  mix_hash(key_hash, &routee_hash.to_le_bytes())
}

fn routee_identity_hash(routee: &Routee) -> u64 {
  match routee {
    | Routee::ActorRef(actor_ref) => {
      let pid = actor_ref.pid();
      let hash = mix_hash(FNV_OFFSET_BASIS, &[0]);
      let hash = mix_hash(hash, &pid.value().to_le_bytes());
      mix_hash(hash, &pid.generation().to_le_bytes())
    },
    | Routee::NoRoutee => mix_hash(FNV_OFFSET_BASIS, &[1]),
    | Routee::Several(routees) => {
      let mut hash = mix_hash(FNV_OFFSET_BASIS, &[2]);
      for routee in routees {
        hash = mix_hash(hash, &routee_identity_hash(routee).to_le_bytes());
      }
      hash
    },
  }
}

pub(crate) fn mix_hash(mut hash: u64, bytes: &[u8]) -> u64 {
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(FNV_PRIME);
  }
  hash
}
