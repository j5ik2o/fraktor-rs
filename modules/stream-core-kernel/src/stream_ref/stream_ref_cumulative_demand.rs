use core::num::NonZeroU64;

/// Cumulative demand signaled by a StreamRef consumer endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamRefCumulativeDemand {
  seq_nr: u64,
  demand: NonZeroU64,
}

impl StreamRefCumulativeDemand {
  /// Creates a cumulative demand protocol payload.
  #[must_use]
  pub const fn new(seq_nr: u64, demand: NonZeroU64) -> Self {
    Self { seq_nr, demand }
  }

  /// Returns the sequence number associated with this demand signal.
  #[must_use]
  pub const fn seq_nr(&self) -> u64 {
    self.seq_nr
  }

  /// Returns the non-zero cumulative demand value.
  #[must_use]
  pub const fn demand(&self) -> NonZeroU64 {
    self.demand
  }
}
