use super::OperatorKey;

/// Compatibility contract for a supported operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorContract {
  /// Stable operator key.
  pub key:                  OperatorKey,
  /// Input-side contract summary.
  pub input_condition:      &'static str,
  /// Completion-side contract summary.
  pub completion_condition: &'static str,
  /// Failure-side contract summary.
  pub failure_condition:    &'static str,
  /// Requirement IDs that define this contract.
  pub requirement_ids:      &'static [&'static str],
}
