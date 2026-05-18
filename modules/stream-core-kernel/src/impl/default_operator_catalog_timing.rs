use crate::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 5] = [
  OperatorContract {
    key:                  OperatorKey::ASYNC_BOUNDARY,
    input_condition:      "Accepts elements while boundary queue has capacity.",
    completion_condition: "Preserves in-island ordering and drains pending boundary elements.",
    failure_condition:    "Backpressures upstream when boundary queue is saturated.",
    requirement_ids:      &["1.1", "1.3", "7.1", "7.2", "7.3", "7.4"],
  },
  OperatorContract {
    key:                  OperatorKey::THROTTLE,
    input_condition:      "Rejects non-positive capacity at construction.",
    completion_condition: "Preserves buffered elements until capacity allows downstream drains.",
    failure_condition:    "Backpressures upstream when capacity is saturated.",
    requirement_ids:      &["1.1", "1.2", "1.3", "7.1", "7.2", "7.3", "7.4"],
  },
  OperatorContract {
    key:                  OperatorKey::DELAY,
    input_condition:      "Rejects non-positive ticks at construction.",
    completion_condition: "Delays each element by configured ticks and drains pending elements on completion.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::INITIAL_DELAY,
    input_condition:      "Rejects non-positive ticks at construction.",
    completion_condition: "Suppresses outputs until initial delay elapses, then drains in order.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::TAKE_WITHIN,
    input_condition:      "Rejects non-positive ticks at construction.",
    completion_condition: "Forwards elements only within configured tick window and then requests shutdown.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
];

/// Coverage entries for timing operators.
pub(super) const COVERAGE: [OperatorCoverage; 5] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
  default_operator_catalog::coverage_for(CONTRACTS[4]),
];

/// Looks up a timing operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns timing operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
