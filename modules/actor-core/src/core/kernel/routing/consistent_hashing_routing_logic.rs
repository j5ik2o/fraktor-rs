//! Consistent-hashing routing logic.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{consistent_hashable_envelope::ConsistentHashableEnvelope, routee::Routee, routing_logic::RoutingLogic};
use crate::core::kernel::actor::messaging::AnyMessage;

// メッセージからハッシュキーを抽出するマッパー型。
// ConsistentHashingPool::create_router 経由で使用される。
type HashKeyMapper = dyn Fn(&AnyMessage) -> u64 + Send + Sync;

/// FNV-1a offset basis used by the routing hash helpers.
pub const FNV_OFFSET_BASIS: u64 = 14695981039346656037;
pub(crate) const FNV_PRIME: u64 = 1099511628211;

/// Selects a routee via rendezvous hashing derived from each message.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.ConsistentHashingRoutingLogic`.
///
/// # Pekko contract
///
/// This logic upholds the four user-visible contracts Pekko guarantees:
///
/// 1. **Stable mapping** — the same hash key is always routed to the same routee as long as the
///    routee set does not change.
/// 2. **Minimal disruption** — when a routee is added or removed, only the keys that would newly
///    prefer the added routee (or previously preferred the removed one) migrate. The expected
///    migration ratio is `1/(n+1)` on addition and `1/n` on removal.
/// 3. **Hash key precedence** — a [`ConsistentHashableEnvelope`] carried by the message takes
///    precedence over the user-supplied `hash_key_mapper` fallback. Types implementing
///    [`ConsistentHashable`](super::ConsistentHashable) directly (without wrapping in an envelope)
///    are not picked up by this dispatcher because [`AnyMessage`] is only downcast to the concrete
///    envelope type; users that need trait-object dispatch on `ConsistentHashable` should wrap the
///    payload in a `ConsistentHashableEnvelope` at the call site, or supply a `hash_key_mapper`
///    that performs the downcast. Native trait-object dispatch may be added in a future revision.
/// 4. **Empty routees** — returns [`Routee::NoRoutee`] without panicking.
///
/// # Design notes
///
/// The selection algorithm uses rendezvous hashing (HRW; Thaler & Ravishankar
/// 1998) instead of Pekko's sorted hash ring. Rendezvous hashing picks the
/// routee with the maximum combined hash of `(key, routee_identity)`, which is
/// provably equivalent to the ring approach for contracts 1–4 while allowing
/// a **stateless** `&self` implementation. This intentionally diverges from
/// Pekko's internal data structures for the following reasons:
///
/// - **No `ConsistentHash<T>` / `MurmurHash` public utilities.** Pekko exposes these as the ring's
///   building blocks. Rendezvous hashing needs neither a sorted map nor Murmur; the 64-bit FNV mix
///   in [`mix_hash`] is sufficient to hash `(key, routee_identity)` pairs deterministically.
///   Re-exposing Pekko's internal helpers would be a Rust copy of an implementation detail (cf.
///   YAGNI in `.agents/rules/rust/reference-implementation.md`).
/// - **No `virtualNodesFactor` parameter.** The ring uses virtual nodes to spread load more evenly
///   across a small sorted map. Rendezvous hashing is already uniform by construction and has no
///   ring to tune, so the parameter would be a no-op knob that misleads users.
/// - **No `AtomicReference` routees cache.** Pekko caches the last-seen `(routees, ring)` pair
///   because rebuilding a sorted ring is `O(n · v)`. Rendezvous selection is `O(n)` per call with
///   no structure to reuse, so the cache is unnecessary — and it would require interior mutability,
///   which is banned by `.agents/rules/rust/immutability-policy.md`.
/// - **No `ConsistentRoutee` wrapper.** Pekko wraps each routee with the `selfAddress` of the node
///   owning it so cluster-remote routees compare correctly. The fraktor-rs [`Routee::ActorRef`]
///   already embeds a unique [`Pid`](crate::core::kernel::actor::Pid) (`value + generation`),
///   making the wrapper unnecessary at this layer.
/// - **`hash_key_mapper` instead of `ConsistentHashMapping`.** Pekko's `ConsistentHashMapping` is
///   `PartialFunction[Any, Any]`; the fraktor-rs equivalent is the `hash_key_mapper:
///   Fn(&AnyMessage) -> u64` closure stored here. It provides the same user-facing hook with a
///   Rust-native signature.
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
  /// Selects a routee deterministically from the message's hash key.
  ///
  /// Extracts the hash key with the following precedence:
  ///
  /// 1. [`ConsistentHashableEnvelope`] carried by the message.
  /// 2. The user-supplied `hash_key_mapper` fallback.
  ///
  /// Types implementing [`ConsistentHashable`](super::ConsistentHashable) directly are not probed
  /// here (see the precedence notes on [`ConsistentHashingRoutingLogic`]); wrap
  /// them in a `ConsistentHashableEnvelope` or handle the downcast inside
  /// `hash_key_mapper`.
  ///
  /// The routee with the maximum rendezvous score is returned; an empty
  /// `routees` slice yields [`Routee::NoRoutee`] per contract 4.
  fn select<'a>(&self, message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    static NO_ROUTEE: Routee = Routee::NoRoutee;
    if routees.is_empty() {
      return &NO_ROUTEE;
    }

    let key_hash = if let Some(envelope) = message.downcast_ref::<ConsistentHashableEnvelope>() {
      envelope.hash_key()
    } else {
      (self.hash_key_mapper)(message)
    };
    let Some((selected_index, _)) =
      routees.iter().enumerate().max_by_key(|(_, routee)| rendezvous_score(key_hash, routee_identity_hash(routee)))
    else {
      return &NO_ROUTEE;
    };

    &routees[selected_index]
  }
}

/// Computes the rendezvous score for a key hash and routee hash pair.
#[must_use]
pub fn rendezvous_score(key_hash: u64, routee_hash: u64) -> u64 {
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

/// FNV-1a mixes a byte slice into an existing 64-bit hash value.
#[must_use]
pub fn mix_hash(mut hash: u64, bytes: &[u8]) -> u64 {
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(FNV_PRIME);
  }
  hash
}
