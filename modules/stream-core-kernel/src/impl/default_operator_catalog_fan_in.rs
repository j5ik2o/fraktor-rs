use crate::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 7] = [
  OperatorContract {
    key:                  OperatorKey::MERGE,
    input_condition:      "Accepts multiple upstream lanes and emits merged output.",
    completion_condition: "Completes when all upstream lanes complete.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::INTERLEAVE,
    input_condition:      "Consumes multiple upstream lanes in round-robin order.",
    completion_condition: "Completes when upstream lanes complete and pending values are drained.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::PREPEND,
    input_condition:      "Consumes lower-index lanes before higher-index lanes.",
    completion_condition: "Completes when all lanes are consumed.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::ZIP,
    input_condition:      "Waits for one element from each upstream lane before emitting.",
    completion_condition: "Completes when upstream lanes complete and pending zip groups are flushed.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::ZIP_ALL,
    input_condition:      "Waits for one element from each upstream lane while active and accepts fill value.",
    completion_condition: "After completion, fills missing lanes and drains remaining pending values.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::ZIP_WITH_INDEX,
    input_condition:      "Pairs each element with an incrementing zero-based index.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::CONCAT,
    input_condition:      "Consumes upstream lanes in deterministic lane order.",
    completion_condition: "Completes after all lanes are consumed.",
    failure_condition:    "Fails when fan-in wiring does not satisfy contract.",
    requirement_ids:      &["1.1", "1.3"],
  },
];

/// Coverage entries for fan-in operators.
pub(super) const COVERAGE: [OperatorCoverage; 7] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
  default_operator_catalog::coverage_for(CONTRACTS[4]),
  default_operator_catalog::coverage_for(CONTRACTS[5]),
  default_operator_catalog::coverage_for(CONTRACTS[6]),
];

/// Looks up a fan-in operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns fan-in operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
