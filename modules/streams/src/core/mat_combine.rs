//! Materialized value combination rules.

/// Rule for combining materialized values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatCombine {
  /// Keep the left materialized value.
  #[default]
  KeepLeft,
  /// Keep the right materialized value.
  KeepRight,
  /// Keep both materialized values.
  KeepBoth,
  /// Drop both materialized values.
  KeepNone,
}

impl MatCombine {
  /// Returns the default combination rule.
  #[must_use]
  pub const fn default_rule() -> Self {
    Self::KeepLeft
  }
}
