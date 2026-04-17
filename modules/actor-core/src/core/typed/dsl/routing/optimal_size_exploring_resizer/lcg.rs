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
  /// The caller must ensure `bound > 0`; passing zero yields an arithmetic
  /// panic, matching Pekko's `Random.nextInt(0)` (which throws
  /// `IllegalArgumentException`). Uses the high 32 bits of `next_u64` to avoid
  /// the well-known low-bit correlation weakness of an LCG, matching the
  /// high-bit extraction done by [`next_f64`](Self::next_f64).
  pub(crate) const fn next_u32_bounded(&mut self, bound: u32) -> u32 {
    ((self.next_u64() >> 32) as u32) % bound
  }
}
