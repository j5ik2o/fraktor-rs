use crate::core::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 4] = [
  OperatorContract {
    key:                  OperatorKey::RECOVER,
    input_condition:      "Consumes the upstream element stream and converts matching upstream failures into one replacement element.",
    completion_condition: "Completes when upstream completes or a matching failure is recovered.",
    failure_condition:    "Propagates unhandled upstream failures.",
    requirement_ids:      &["1.1", "1.3", "3.4"],
  },
  OperatorContract {
    key:                  OperatorKey::RECOVER_WITH_RETRIES,
    input_condition:      "Switches to alternate recovery sources while retry budget remains.",
    completion_condition: "Completes when the active stream path (upstream or recovery source) completes.",
    failure_condition:    "Fails when retry budget is exhausted or recovery source creation fails.",
    requirement_ids:      &["1.1", "1.3", "3.4"],
  },
  OperatorContract {
    key:                  OperatorKey::RESTART,
    input_condition:      "Schedules restart on stage failure/completion while restart budget remains.",
    completion_condition: "Completes on max-restart exhaustion when complete-on-exhaustion is enabled.",
    failure_condition:    "Fails on max-restart exhaustion when fail-on-exhaustion is configured.",
    requirement_ids:      &["1.1", "1.3", "6.1", "6.2", "6.3"],
  },
  OperatorContract {
    key:                  OperatorKey::SUPERVISION,
    input_condition:      "Applies stop/resume/restart directive to stage failures.",
    completion_condition: "Keeps stream alive for resume/restart directives.",
    failure_condition:    "Fails stream when stop directive is selected.",
    requirement_ids:      &["1.1", "1.3", "6.4", "6.5", "6.6"],
  },
];

/// Coverage entries for failure operators.
pub(super) const COVERAGE: [OperatorCoverage; 4] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
];

/// Looks up a failure operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns failure operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
