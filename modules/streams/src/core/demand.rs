//! Demand tracking primitives.

/// Demand value requested by downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Demand {
  /// Finite demand amount.
  Finite(u64),
  /// Unbounded demand.
  Unbounded,
}

impl Demand {
  /// Returns true when demand permits at least one element.
  #[must_use]
  pub const fn has_demand(self) -> bool {
    matches!(self, Demand::Finite(value) if value > 0) || matches!(self, Demand::Unbounded)
  }
}
