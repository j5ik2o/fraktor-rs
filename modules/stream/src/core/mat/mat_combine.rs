/// Materialized value combine strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatCombine {
  /// Keeps the left materialized value.
  Left,
  /// Keeps the right materialized value.
  Right,
  /// Keeps both materialized values.
  Both,
  /// Drops both materialized values.
  Neither,
}
