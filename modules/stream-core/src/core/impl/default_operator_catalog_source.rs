use crate::core::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 4] = [
  OperatorContract {
    key:                  OperatorKey::EMPTY,
    input_condition:      "Emits no elements and does not pull from upstream.",
    completion_condition: "Completes immediately after materialization.",
    failure_condition:    "Does not fail unless stream setup fails.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FROM_OPTION,
    input_condition:      "Materializes to a single-element source for Some or an empty source for None.",
    completion_condition: "Completes immediately after emitting zero or one element.",
    failure_condition:    "Does not fail unless stream setup fails.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FROM_ARRAY,
    input_condition:      "Emits array elements in order on downstream demand.",
    completion_condition: "Completes after all array elements are emitted.",
    failure_condition:    "Does not fail unless stream setup fails.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FROM_ITERATOR,
    input_condition:      "Pulls the next element from iterator on downstream demand.",
    completion_condition: "Completes when iterator is exhausted.",
    failure_condition:    "Does not fail unless stream setup fails.",
    requirement_ids:      &["1.1", "1.3"],
  },
];

/// Coverage entries for source operators.
pub(super) const COVERAGE: [OperatorCoverage; 4] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
];

/// Looks up a source operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns source operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
