use crate::core::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 19] = [
  OperatorContract {
    key:                  OperatorKey::BUFFER,
    input_condition:      "Rejects non-positive capacity at construction.",
    completion_condition: "Preserves buffered elements until source completion is drained.",
    failure_condition:    "Fails with overflow semantics according to configured policy.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::BATCH,
    input_condition:      "Rejects non-positive group size at construction.",
    completion_condition: "Flushes trailing partial group on upstream completion.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FILTER,
    input_condition:      "Evaluates each element against predicate and forwards only matches.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FILTER_NOT,
    input_condition:      "Evaluates each element against predicate and forwards only non-matches.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::MAP_CONCAT,
    input_condition:      "Expands each element into zero or more elements while preserving order.",
    completion_condition: "Completes when upstream completes and expanded elements are emitted.",
    failure_condition:    "Propagates upstream or mapper failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::MAP_ASYNC,
    input_condition:      "Maps each element using provided closure in asynchronous-map compatibility mode.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or mapper failures.",
    requirement_ids:      &["1.1", "1.3", "7.1", "7.2", "7.3", "7.4"],
  },
  OperatorContract {
    key:                  OperatorKey::MAP_OPTION,
    input_condition:      "Maps each element and emits only present values.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or mapper failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::FLATTEN_OPTIONAL,
    input_condition:      "Accepts optional payloads and emits only present values.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::STATEFUL_MAP,
    input_condition:      "Creates a mapper from factory and applies it in element order.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream, factory, or mapper failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::STATEFUL_MAP_CONCAT,
    input_condition:      "Creates a map-concat mapper from factory and expands each element in order.",
    completion_condition: "Completes when upstream completes and expanded elements are drained.",
    failure_condition:    "Propagates upstream, factory, or mapper failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::DROP,
    input_condition:      "Skips the first configured number of elements.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::TAKE,
    input_condition:      "Emits at most the configured number of elements in arrival order.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::DROP_WHILE,
    input_condition:      "Drops prefix elements while predicate matches and then forwards all remaining elements.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::TAKE_WHILE,
    input_condition:      "Emits prefix elements while predicate matches and discards subsequent elements.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::TAKE_UNTIL,
    input_condition:      "Emits elements until predicate first matches and includes the matching element.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream or predicate evaluation failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::GROUPED,
    input_condition:      "Rejects non-positive group size at construction.",
    completion_condition: "Flushes trailing partial group on upstream completion.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::INTERSPERSE,
    input_condition:      "Injects start/element-separator/end markers in deterministic order.",
    completion_condition: "Emits start/end markers even when upstream is empty.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::SLIDING,
    input_condition:      "Rejects non-positive window size at construction.",
    completion_condition: "Completes when upstream completes.",
    failure_condition:    "Propagates upstream failures.",
    requirement_ids:      &["1.1", "1.2", "1.3"],
  },
  OperatorContract {
    key:                  OperatorKey::SCAN,
    input_condition:      "Emits initial value and then running accumulation for each element.",
    completion_condition: "Emits initial value even when upstream is empty.",
    failure_condition:    "Propagates upstream or accumulator failures.",
    requirement_ids:      &["1.1", "1.3"],
  },
];

/// Coverage entries for transform operators.
pub(super) const COVERAGE: [OperatorCoverage; 19] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
  default_operator_catalog::coverage_for(CONTRACTS[4]),
  default_operator_catalog::coverage_for(CONTRACTS[5]),
  default_operator_catalog::coverage_for(CONTRACTS[6]),
  default_operator_catalog::coverage_for(CONTRACTS[7]),
  default_operator_catalog::coverage_for(CONTRACTS[8]),
  default_operator_catalog::coverage_for(CONTRACTS[9]),
  default_operator_catalog::coverage_for(CONTRACTS[10]),
  default_operator_catalog::coverage_for(CONTRACTS[11]),
  default_operator_catalog::coverage_for(CONTRACTS[12]),
  default_operator_catalog::coverage_for(CONTRACTS[13]),
  default_operator_catalog::coverage_for(CONTRACTS[14]),
  default_operator_catalog::coverage_for(CONTRACTS[15]),
  default_operator_catalog::coverage_for(CONTRACTS[16]),
  default_operator_catalog::coverage_for(CONTRACTS[17]),
  default_operator_catalog::coverage_for(CONTRACTS[18]),
];

/// Looks up a transform operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns transform operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
