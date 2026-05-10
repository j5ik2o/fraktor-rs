use crate::r#impl::{OperatorContract, OperatorCoverage, OperatorKey, default_operator_catalog};

const CONTRACTS: [OperatorContract; 8] = [
  OperatorContract {
    key:                  OperatorKey::FLAT_MAP_CONCAT,
    input_condition:      "Starts next inner stream only after current inner stream completes.",
    completion_condition: "Completes after upstream completion and all inner streams completion.",
    failure_condition:    "Fails the whole stream when an inner stream fails without recovery.",
    requirement_ids:      &["1.1", "1.3", "3.1"],
  },
  OperatorContract {
    key:                  OperatorKey::FLAT_MAP_MERGE,
    input_condition:      "Enforces breadth limit and suppresses upstream pulls at saturation.",
    completion_condition: "Completes after upstream completion and active inner streams drain.",
    failure_condition:    "Fails the whole stream when an inner stream fails without recovery.",
    requirement_ids:      &["1.1", "1.3", "3.2", "3.3", "3.4"],
  },
  OperatorContract {
    key:                  OperatorKey::GROUP_BY,
    input_condition:      "Rejects non-positive max_substreams at construction.",
    completion_condition: "Routes each element to a key lane while key count stays within limit.",
    failure_condition:    "Fails when observed unique key count exceeds configured max_substreams.",
    requirement_ids:      &["1.1", "1.3", "2.1", "2.2"],
  },
  OperatorContract {
    key:                  OperatorKey::SPLIT_WHEN,
    input_condition:      "Starts a new segment with the matching element.",
    completion_condition: "Emits all completed segments and flushes trailing segment on source completion.",
    failure_condition:    "Propagates upstream/inner stage failures.",
    requirement_ids:      &["1.1", "1.3", "2.3"],
  },
  OperatorContract {
    key:                  OperatorKey::SPLIT_AFTER,
    input_condition:      "Keeps the matching element at the tail of the current segment.",
    completion_condition: "Emits all completed segments and flushes trailing segment on source completion.",
    failure_condition:    "Propagates upstream/inner stage failures.",
    requirement_ids:      &["1.1", "1.3", "2.4"],
  },
  OperatorContract {
    key:                  OperatorKey::MERGE_SUBSTREAMS,
    input_condition:      "Accepts segmented substream payloads and merges with unbounded parallelism semantics.",
    completion_condition: "Emits all elements from completed segments without loss.",
    failure_condition:    "Fails on invalid substream payload type.",
    requirement_ids:      &["1.1", "1.3", "2.5"],
  },
  OperatorContract {
    key:                  OperatorKey::MERGE_SUBSTREAMS_WITH_PARALLELISM,
    input_condition:      "Rejects non-positive parallelism at construction.",
    completion_condition: "Emits all elements from completed segments without loss.",
    failure_condition:    "Fails on invalid substream payload type.",
    requirement_ids:      &["1.1", "1.2", "1.3", "2.5"],
  },
  OperatorContract {
    key:                  OperatorKey::CONCAT_SUBSTREAMS,
    input_condition:      "Concatenates substreams with sequential semantics.",
    completion_condition: "Emits all elements in segment order without loss.",
    failure_condition:    "Fails on invalid substream payload type.",
    requirement_ids:      &["1.1", "1.3", "2.5"],
  },
];

/// Coverage entries for substream operators.
pub(super) const COVERAGE: [OperatorCoverage; 8] = [
  default_operator_catalog::coverage_for(CONTRACTS[0]),
  default_operator_catalog::coverage_for(CONTRACTS[1]),
  default_operator_catalog::coverage_for(CONTRACTS[2]),
  default_operator_catalog::coverage_for(CONTRACTS[3]),
  default_operator_catalog::coverage_for(CONTRACTS[4]),
  default_operator_catalog::coverage_for(CONTRACTS[5]),
  default_operator_catalog::coverage_for(CONTRACTS[6]),
  default_operator_catalog::coverage_for(CONTRACTS[7]),
];

/// Looks up a substream operator contract.
#[must_use]
pub(super) fn lookup(key: OperatorKey) -> Option<OperatorContract> {
  CONTRACTS.iter().find(|contract| contract.key == key).copied()
}

/// Returns substream operator coverage.
#[must_use]
pub(super) const fn coverage() -> &'static [OperatorCoverage] {
  &COVERAGE
}
