use super::{OperatorContract, OperatorCoverage, OperatorKey, StreamDslError};

/// Contract for operator catalog lookup.
pub trait OperatorCatalog {
  /// Looks up an operator contract for a key.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError::UnsupportedOperator`] when the operator is not covered.
  fn lookup(&self, key: OperatorKey) -> Result<OperatorContract, StreamDslError>;

  /// Returns coverage metadata for all operators in this catalog.
  fn coverage(&self) -> &'static [OperatorCoverage];
}
