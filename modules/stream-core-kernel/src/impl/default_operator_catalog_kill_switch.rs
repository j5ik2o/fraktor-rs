use crate::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 2] = [
  OperatorContract {
    key:                  OperatorKey::UNIQUE_KILL_SWITCH,
    input_condition:      "Applies first shutdown or abort control signal once.",
    completion_condition: "Shutdown cancels upstream and completes downstream.",
    failure_condition:    "Abort cancels upstream and fails downstream with provided error.",
    requirement_ids:      &["1.1", "1.3", "5.1", "5.2", "5.3"],
  },
  OperatorContract {
    key:                  OperatorKey::SHARED_KILL_SWITCH,
    input_condition:      "Allows creating shared control before stream materialization.",
    completion_condition: "Shutdown completes all linked streams.",
    failure_condition:    "Abort fails all linked streams with first error.",
    requirement_ids:      &["1.1", "1.3", "5.3", "5.4", "5.5"],
  },
];

/// Coverage entries for kill-switch operators.
pub(super) const COVERAGE: [OperatorCoverage; 2] =
  [default_operator_catalog::coverage_for(CONTRACTS[0]), default_operator_catalog::coverage_for(CONTRACTS[1])];

/// Looks up a kill-switch operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns kill-switch operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
