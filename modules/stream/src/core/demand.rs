/// Demand signal used for backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demand {
  /// Finite demand with remaining count.
  Finite(u64),
  /// Unbounded demand.
  Unbounded,
}

impl Demand {
  /// Returns `true` if the demand is unbounded.
  #[must_use]
  pub const fn is_unbounded(&self) -> bool {
    matches!(self, Self::Unbounded)
  }

  /// Returns `true` if there is remaining demand.
  #[must_use]
  pub const fn has_demand(&self) -> bool {
    matches!(self, Self::Unbounded) || matches!(self, Self::Finite(remaining) if *remaining > 0)
  }

  /// Returns the remaining finite demand, if any.
  #[must_use]
  pub const fn remaining(&self) -> Option<u64> {
    match self {
      | Self::Finite(value) => Some(*value),
      | Self::Unbounded => None,
    }
  }
}
