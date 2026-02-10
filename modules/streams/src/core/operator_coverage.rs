use super::OperatorKey;

/// Requirement coverage metadata for a compatibility operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorCoverage {
  /// Stable operator key.
  pub key:             OperatorKey,
  /// Requirement IDs linked to this operator.
  pub requirement_ids: &'static [&'static str],
}
