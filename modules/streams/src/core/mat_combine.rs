/// Materialized value combine strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatCombine {
  /// Keeps the left materialized value.
  KeepLeft,
  /// Keeps the right materialized value.
  KeepRight,
  /// Keeps both materialized values.
  KeepBoth,
  /// Drops both materialized values.
  KeepNone,
}
