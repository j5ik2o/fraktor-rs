use crate::core::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 3] = [
  OperatorContract {
    key:                  OperatorKey::MERGE_HUB,
    input_condition:      "Accepts offers only after receiver side is activated.",
    completion_condition: "Drains queued offers through source side polling.",
    failure_condition:    "Backpressures offers when receiver is inactive or queue is full.",
    requirement_ids:      &["1.1", "1.3", "4.1", "4.2"],
  },
  OperatorContract {
    key:                  OperatorKey::BROADCAST_HUB,
    input_condition:      "Delivers each element to all active subscribers.",
    completion_condition: "Keeps source pull pending until new publish arrives.",
    failure_condition:    "Backpressures when no subscriber exists or queues are full.",
    requirement_ids:      &["1.1", "1.3", "4.2", "4.3"],
  },
  OperatorContract {
    key:                  OperatorKey::PARTITION_HUB,
    input_condition:      "Routes each element to exactly one active partition.",
    completion_condition: "Drains partition queue in partition order.",
    failure_condition:    "Backpressures without active consumer and rejects invalid route values.",
    requirement_ids:      &["1.1", "1.3", "4.4", "4.5"],
  },
];

/// Coverage entries for hub operators.
pub(super) const COVERAGE: [OperatorCoverage; 3] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
];

/// Looks up a hub operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns hub operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
