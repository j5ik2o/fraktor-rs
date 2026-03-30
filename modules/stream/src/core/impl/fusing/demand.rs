/// Demand signal used for backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Demand {
  /// Finite demand with remaining count.
  Finite(u64),
  /// Unbounded demand.
  Unbounded,
}

impl Demand {
  /// Returns `true` if there is remaining demand.
  #[must_use]
  pub(crate) const fn has_demand(&self) -> bool {
    matches!(self, Self::Unbounded) || matches!(self, Self::Finite(remaining) if *remaining > 0)
  }
}
