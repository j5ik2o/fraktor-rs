use alloc::{string::String, vec, vec::Vec};

use crate::{
  ClusterProviderError,
  cluster_provider::{SeedNodeInput, SeedNodeProcess},
};

fn input(advertised_authority: &str, seed_authorities: Vec<&str>) -> SeedNodeInput {
  SeedNodeInput::new(String::from(advertised_authority), seed_authorities.into_iter().map(String::from).collect())
}

#[test]
fn seed_node_process_member_start_reports_empty_seed_without_failure() {
  let mut process = SeedNodeProcess::new();

  let joins = process.start_member(&input("node-a", vec![])).unwrap();

  assert!(joins.is_empty());
}

#[test]
fn seed_node_process_member_start_filters_self_authority() {
  let mut process = SeedNodeProcess::new();

  let joins = process.start_member(&input("node-a", vec!["node-a", "node-b"])).unwrap();

  assert_eq!(joins, vec![String::from("node-b")]);
}

#[test]
fn seed_node_process_member_start_deduplicates_seed_authorities() {
  let mut process = SeedNodeProcess::new();

  let joins = process.start_member(&input("node-a", vec!["node-b", "node-b", "node-c"])).unwrap();

  assert_eq!(joins, vec![String::from("node-b"), String::from("node-c")]);
}

#[test]
fn seed_node_process_member_start_reports_invalid_authority() {
  let mut process = SeedNodeProcess::new();

  let error = process.start_member(&input("node-a", vec!["node-b", ""])).unwrap_err();

  assert_eq!(error, ClusterProviderError::join("invalid seed authority"));
}

#[test]
fn seed_node_process_client_start_does_not_emit_member_join_input() {
  let mut process = SeedNodeProcess::new();

  let joins = process.start_client(&input("node-a", vec!["node-b"])).unwrap();

  assert!(joins.is_empty());
}

#[test]
fn seed_node_process_stops_join_input_after_shutdown() {
  let mut process = SeedNodeProcess::new();
  let initial = process.start_member(&input("node-a", vec!["node-b"])).unwrap();
  process.shutdown().unwrap();

  let joins = process.start_member(&input("node-a", vec!["node-c"])).unwrap();

  assert_eq!(initial, vec![String::from("node-b")]);
  assert!(joins.is_empty());
}
