use super::MatCombine;

#[cfg(test)]
mod tests;

/// Type-level rule for combining materialized values.
pub trait MatCombineRule<Left, Right> {
  /// Output type produced by the combination.
  type Out;

  /// Returns the combination kind.
  fn kind() -> MatCombine;

  /// Combines materialized values according to the rule.
  fn combine(left: Left, right: Right) -> Self::Out;
}
