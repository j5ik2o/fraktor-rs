//! Random routing logic.

#[cfg(test)]
mod tests;

use portable_atomic::{AtomicU64, Ordering};

use crate::core::kernel::{
  actor::messaging::AnyMessage,
  routing::{Routee, RoutingLogic},
};

/// Selects a routee at random using a seed-based xorshift64 PRNG.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RandomRoutingLogic`.
///
/// The PRNG state is managed via an atomic counter so that `select` can be
/// called via `&self` concurrently. Given the same seed, the selection
/// sequence is deterministic.
pub struct RandomRoutingLogic {
  state: AtomicU64,
}

impl RandomRoutingLogic {
  /// Creates a new random logic seeded with the given value.
  #[must_use]
  pub const fn new(seed: u64) -> Self {
    // Ensure the initial state is never zero (xorshift64 would degenerate).
    let initial = if seed == 0 { 1 } else { seed };
    Self { state: AtomicU64::new(initial) }
  }

  /// Advances the xorshift64 state and returns the next pseudo-random value.
  fn next_u64(&self) -> u64 {
    loop {
      let current = self.state.load(Ordering::Relaxed);
      let mut x = current;
      x ^= x << 13;
      x ^= x >> 7;
      x ^= x << 17;
      if self.state.compare_exchange_weak(current, x, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        return x;
      }
    }
  }
}

impl RoutingLogic for RandomRoutingLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    if routees.is_empty() {
      static NO_ROUTEE: Routee = Routee::NoRoutee;
      return &NO_ROUTEE;
    }
    let idx = (self.next_u64() as usize) % routees.len();
    &routees[idx]
  }
}
