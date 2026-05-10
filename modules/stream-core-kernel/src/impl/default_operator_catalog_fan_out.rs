use crate::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 5] = [
  OperatorContract {
    key:                  OperatorKey::BROADCAST,
    input_condition:      "Duplicates each element to all connected downstream lanes.",
    completion_condition: "Completes when upstream completes and all duplicates are drained.",
    failure_condition:    "Fails when fan-out contract is invalid.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::BALANCE,
    input_condition:      "Distributes each element to one downstream lane in round-robin order.",
    completion_condition: "Completes when upstream completes and buffered elements are drained.",
    failure_condition:    "Fails when fan-out contract is invalid.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::PARTITION,
    input_condition:      "Routes each element into one of two lanes according to predicate.",
    completion_condition: "Completes when upstream completes and routed elements are drained.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::UNZIP,
    input_condition:      "Accepts tuple payloads and routes tuple components to two output lanes.",
    completion_condition: "Completes when upstream completes and both lanes are drained.",
    failure_condition:    "Fails on non-tuple payload type mismatch.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::UNZIP_WITH,
    input_condition:      "Maps each payload to a tuple and routes each component to its lane.",
    completion_condition: "Completes when upstream completes and both mapped lanes are drained.",
    failure_condition:    "Propagates upstream or mapper failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
];

/// Coverage entries for fan-out operators.
pub(super) const COVERAGE: [OperatorCoverage; 5] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
  default_operator_catalog::coverage_for(CONTRACTS[4]),
];

/// Looks up a fan-out operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns fan-out operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
