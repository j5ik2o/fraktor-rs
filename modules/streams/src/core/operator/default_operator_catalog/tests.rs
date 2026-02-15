use crate::core::{
  StreamDslError,
  operator::{DefaultOperatorCatalog, OperatorCatalog, OperatorKey},
};

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
fn coverage_extends_beyond_nine_operators() {
  let catalog = DefaultOperatorCatalog::new();
  assert!(catalog.coverage().len() > 9);
}
