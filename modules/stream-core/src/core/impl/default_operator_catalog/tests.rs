use crate::core::{
  StreamDslError,
  r#impl::{
    DefaultOperatorCatalog, OperatorCatalog, OperatorCoverage, OperatorKey,
    default_operator_catalog_failure as failure, default_operator_catalog_fan_in as fan_in,
    default_operator_catalog_fan_out as fan_out, default_operator_catalog_hub as hub,
    default_operator_catalog_kill_switch as kill_switch, default_operator_catalog_source as source,
    default_operator_catalog_substream as substream, default_operator_catalog_timing as timing,
    default_operator_catalog_transform as transform,
  },
};

const EXPECTED_OPERATOR_COUNT: usize = 57;

fn category_coverage_slices() -> [&'static [OperatorCoverage]; 9] {
  [
    source::coverage(),
    transform::coverage(),
    substream::coverage(),
    timing::coverage(),
    fan_in::coverage(),
    fan_out::coverage(),
    failure::coverage(),
    hub::coverage(),
    kill_switch::coverage(),
  ]
}

fn assert_keys(actual: &[OperatorCoverage], expected: &[OperatorKey]) {
  assert_eq!(actual.len(), expected.len());
  for key in expected {
    assert!(actual.iter().any(|entry| entry.key == *key), "missing operator coverage for {key:?}");
  }
}

#[test]
fn lookup_returns_group_by_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::GROUP_BY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::GROUP_BY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "2.1", "2.2"]);
}

#[test]
fn lookup_rejects_unknown_operator() {
  let catalog = DefaultOperatorCatalog::new();
  let key = OperatorKey::new("unknown");
  let result = catalog.lookup(key);
  assert_eq!(result, Err(StreamDslError::UnsupportedOperator { key }));
}

#[test]
fn coverage_contains_merge_substreams_with_parallelism() {
  let catalog = DefaultOperatorCatalog::new();
  let covered = catalog.coverage().iter().any(|entry| entry.key == OperatorKey::MERGE_SUBSTREAMS_WITH_PARALLELISM);
  assert!(covered);
}

#[test]
fn lookup_returns_async_boundary_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::ASYNC_BOUNDARY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::ASYNC_BOUNDARY);
  assert!(contract.requirement_ids.contains(&"7.4"));
}

#[test]
fn lookup_returns_map_async_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::MAP_ASYNC).expect("lookup");
  assert_eq!(contract.key, OperatorKey::MAP_ASYNC);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "7.1", "7.2", "7.3", "7.4"]);
}

#[test]
fn lookup_returns_batch_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::BATCH).expect("lookup");
  assert_eq!(contract.key, OperatorKey::BATCH);
  assert_eq!(contract.requirement_ids, &["1.1", "1.2", "1.3"]);
}

#[test]
fn lookup_returns_throttle_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::THROTTLE).expect("lookup");
  assert_eq!(contract.key, OperatorKey::THROTTLE);
  assert!(contract.requirement_ids.contains(&"1.3"));
}

#[test]
fn lookup_returns_delay_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::DELAY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::DELAY);
  assert!(contract.requirement_ids.contains(&"1.2"));
}

#[test]
fn lookup_returns_take_within_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::TAKE_WITHIN).expect("lookup");
  assert_eq!(contract.key, OperatorKey::TAKE_WITHIN);
  assert!(contract.requirement_ids.contains(&"1.3"));
}

#[test]
fn lookup_returns_partition_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::PARTITION).expect("lookup");
  assert_eq!(contract.key, OperatorKey::PARTITION);
  assert!(contract.requirement_ids.contains(&"1.1"));
}

#[test]
fn lookup_returns_zip_all_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::ZIP_ALL).expect("lookup");
  assert_eq!(contract.key, OperatorKey::ZIP_ALL);
  assert!(contract.requirement_ids.contains(&"1.3"));
}

#[test]
fn coverage_contains_batch() {
  let catalog = DefaultOperatorCatalog::new();
  let covered = catalog.coverage().iter().any(|entry| entry.key == OperatorKey::BATCH);
  assert!(covered);
}

#[test]
fn coverage_contains_throttle() {
  let catalog = DefaultOperatorCatalog::new();
  let covered = catalog.coverage().iter().any(|entry| entry.key == OperatorKey::THROTTLE);
  assert!(covered);
}

#[test]
fn coverage_contains_delay() {
  let catalog = DefaultOperatorCatalog::new();
  let covered = catalog.coverage().iter().any(|entry| entry.key == OperatorKey::DELAY);
  assert!(covered);
}

#[test]
fn coverage_contains_partition() {
  let catalog = DefaultOperatorCatalog::new();
  let covered = catalog.coverage().iter().any(|entry| entry.key == OperatorKey::PARTITION);
  assert!(covered);
}

#[test]
fn coverage_extends_beyond_nine_operators() {
  let catalog = DefaultOperatorCatalog::new();
  assert!(catalog.coverage().len() > 9);
}

#[test]
fn source_transform_and_substream_coverage_are_separated_by_operator_family() {
  assert_keys(source::coverage(), &[
    OperatorKey::EMPTY,
    OperatorKey::FROM_OPTION,
    OperatorKey::FROM_ARRAY,
    OperatorKey::FROM_ITERATOR,
  ]);
  assert_keys(transform::coverage(), &[
    OperatorKey::BUFFER,
    OperatorKey::BATCH,
    OperatorKey::FILTER,
    OperatorKey::FILTER_NOT,
    OperatorKey::MAP_CONCAT,
    OperatorKey::MAP_ASYNC,
    OperatorKey::MAP_OPTION,
    OperatorKey::FLATTEN_OPTIONAL,
    OperatorKey::STATEFUL_MAP,
    OperatorKey::STATEFUL_MAP_CONCAT,
    OperatorKey::DROP,
    OperatorKey::TAKE,
    OperatorKey::DROP_WHILE,
    OperatorKey::TAKE_WHILE,
    OperatorKey::TAKE_UNTIL,
    OperatorKey::GROUPED,
    OperatorKey::INTERSPERSE,
    OperatorKey::SLIDING,
    OperatorKey::SCAN,
  ]);
  assert_keys(substream::coverage(), &[
    OperatorKey::FLAT_MAP_CONCAT,
    OperatorKey::FLAT_MAP_MERGE,
    OperatorKey::GROUP_BY,
    OperatorKey::SPLIT_WHEN,
    OperatorKey::SPLIT_AFTER,
    OperatorKey::MERGE_SUBSTREAMS,
    OperatorKey::MERGE_SUBSTREAMS_WITH_PARALLELISM,
    OperatorKey::CONCAT_SUBSTREAMS,
  ]);
}

#[test]
fn timing_fan_in_and_fan_out_coverage_are_separated_by_operator_family() {
  assert_keys(timing::coverage(), &[
    OperatorKey::ASYNC_BOUNDARY,
    OperatorKey::THROTTLE,
    OperatorKey::DELAY,
    OperatorKey::INITIAL_DELAY,
    OperatorKey::TAKE_WITHIN,
  ]);
  assert_keys(fan_in::coverage(), &[
    OperatorKey::MERGE,
    OperatorKey::INTERLEAVE,
    OperatorKey::PREPEND,
    OperatorKey::ZIP,
    OperatorKey::ZIP_ALL,
    OperatorKey::ZIP_WITH_INDEX,
    OperatorKey::CONCAT,
  ]);
  assert_keys(fan_out::coverage(), &[
    OperatorKey::BROADCAST,
    OperatorKey::BALANCE,
    OperatorKey::PARTITION,
    OperatorKey::UNZIP,
    OperatorKey::UNZIP_WITH,
  ]);
}

#[test]
fn failure_hub_and_kill_switch_coverage_are_separated_by_operator_family() {
  assert_keys(failure::coverage(), &[
    OperatorKey::RECOVER,
    OperatorKey::RECOVER_WITH_RETRIES,
    OperatorKey::RESTART,
    OperatorKey::SUPERVISION,
  ]);
  assert_keys(hub::coverage(), &[OperatorKey::MERGE_HUB, OperatorKey::BROADCAST_HUB, OperatorKey::PARTITION_HUB]);
  assert_keys(kill_switch::coverage(), &[OperatorKey::UNIQUE_KILL_SWITCH, OperatorKey::SHARED_KILL_SWITCH]);
}

#[test]
fn category_coverage_is_complete_and_has_no_duplicate_operator_keys() {
  let category_coverage = category_coverage_slices();
  let total = category_coverage.iter().map(|entries| entries.len()).sum::<usize>();
  assert_eq!(total, EXPECTED_OPERATOR_COUNT);

  for (category_index, entries) in category_coverage.iter().enumerate() {
    for (entry_index, entry) in entries.iter().enumerate() {
      for later_entry in entries.iter().skip(entry_index + 1) {
        assert_ne!(entry.key, later_entry.key);
      }
      for later_category in category_coverage.iter().skip(category_index + 1) {
        assert!(!later_category.iter().any(|later_entry| later_entry.key == entry.key));
      }
    }
  }
}

#[test]
fn category_coverage_requirement_ids_match_default_lookup_contracts() {
  let catalog = DefaultOperatorCatalog::new();

  for entries in category_coverage_slices() {
    for entry in entries {
      let contract = catalog.lookup(entry.key).expect("category coverage key must be handled by default catalog");
      assert_eq!(contract.requirement_ids, entry.requirement_ids);
    }
  }
}

#[test]
fn default_catalog_coverage_matches_category_coverage() {
  let catalog = DefaultOperatorCatalog::new();
  let default_coverage = catalog.coverage();
  assert_eq!(default_coverage.len(), EXPECTED_OPERATOR_COUNT);

  for entries in category_coverage_slices() {
    for entry in entries {
      assert!(default_coverage.iter().any(|default_entry| default_entry == entry));
    }
  }
}

#[test]
fn category_lookup_returns_none_for_operators_owned_by_other_categories() {
  assert!(source::lookup(OperatorKey::MERGE).is_none());
  assert!(transform::lookup(OperatorKey::GROUP_BY).is_none());
  assert!(substream::lookup(OperatorKey::THROTTLE).is_none());
  assert!(timing::lookup(OperatorKey::BROADCAST).is_none());
  assert!(fan_in::lookup(OperatorKey::PARTITION).is_none());
  assert!(fan_out::lookup(OperatorKey::RECOVER).is_none());
  assert!(failure::lookup(OperatorKey::MERGE_HUB).is_none());
  assert!(hub::lookup(OperatorKey::UNIQUE_KILL_SWITCH).is_none());
  assert!(kill_switch::lookup(OperatorKey::EMPTY).is_none());
}

#[test]
fn category_lookup_preserves_representative_pekko_contracts() {
  let source_contract = source::lookup(OperatorKey::EMPTY).expect("source contract");
  assert_eq!(source_contract.completion_condition, "Completes immediately after materialization.");

  let transform_contract = transform::lookup(OperatorKey::BATCH).expect("transform contract");
  assert_eq!(transform_contract.requirement_ids, &["1.1", "1.2", "1.3"]);

  let substream_contract = substream::lookup(OperatorKey::FLAT_MAP_MERGE).expect("substream contract");
  assert_eq!(substream_contract.requirement_ids, &["1.1", "1.3", "3.2", "3.3", "3.4"]);

  let timing_contract = timing::lookup(OperatorKey::THROTTLE).expect("timing contract");
  assert_eq!(timing_contract.requirement_ids, &["1.1", "1.2", "1.3", "7.1", "7.2", "7.3", "7.4"]);

  let fan_in_contract = fan_in::lookup(OperatorKey::ZIP_ALL).expect("fan-in contract");
  assert_eq!(
    fan_in_contract.input_condition,
    "Waits for one element from each upstream lane while active and accepts fill value."
  );

  let fan_out_contract = fan_out::lookup(OperatorKey::BROADCAST).expect("fan-out contract");
  assert_eq!(fan_out_contract.input_condition, "Duplicates each element to all connected downstream lanes.");

  let failure_contract = failure::lookup(OperatorKey::RESTART).expect("failure contract");
  assert_eq!(failure_contract.requirement_ids, &["1.1", "1.3", "6.1", "6.2", "6.3"]);

  let hub_contract = hub::lookup(OperatorKey::PARTITION_HUB).expect("hub contract");
  assert_eq!(hub_contract.requirement_ids, &["1.1", "1.3", "4.4", "4.5"]);

  let kill_switch_contract = kill_switch::lookup(OperatorKey::UNIQUE_KILL_SWITCH).expect("kill-switch contract");
  assert_eq!(kill_switch_contract.requirement_ids, &["1.1", "1.3", "5.1", "5.2", "5.3"]);
}
