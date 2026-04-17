//! Seedable LCG pseudo-random generator.
//!
//! Replaces Pekko's `ThreadLocalRandom` in the `OptimalSizeExploringResizer`
//! algorithm so that explore / optimize branching is deterministic under a
//! fixed seed. Uses the Numerical Recipes MMIX constants.

/// Linear congruential generator with 64-bit state.
pub(crate) struct Lcg {
  state: u64,
}

impl Lcg {
  /// Creates a new generator seeded with `seed`.
  pub(crate) const fn new(seed: u64) -> Self {
    Self { state: seed }
  }

  /// Advances the state and returns the raw 64-bit value.
  const fn next_u64(&mut self) -> u64 {
    // Numerical Recipes MMIX constants.
    self.state = self.state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    self.state
  }

  /// Returns a uniformly distributed value in `[0, 1)`.
  ///
  /// Uses the top 53 bits of the internal state, matching the precision of
  /// `f64` mantissa.
  pub(crate) fn next_f64(&mut self) -> f64 {
    let bits = self.next_u64() >> 11;
    let denom = (1_u64 << 53) as f64;
    bits as f64 / denom
  }

  /// Returns a uniformly distributed integer in `[0, bound)`.
  ///
  /// Returns `0` when `bound == 0` (matching our caller's contract — Pekko's
  /// `Random.nextInt(0)` would throw, which we avoid by treating zero as a
  /// degenerate range).
  pub(crate) const fn next_u32_bounded(&mut self, bound: u32) -> u32 {
    if bound == 0 {
      return 0;
    }
    (self.next_u64() as u32) % bound
  }
}
