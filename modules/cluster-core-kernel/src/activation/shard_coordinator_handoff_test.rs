use alloc::{collections::BTreeSet, string::String};

use super::ShardCoordinatorHandoff;
use crate::activation::{
  shard_coordinator_handoff_action::ShardCoordinatorHandoffAction,
  shard_coordinator_handoff_command::ShardCoordinatorHandoffCommand,
  shard_coordinator_handoff_outcome::ShardCoordinatorHandoffOutcome,
};

fn regions(values: &[&str]) -> BTreeSet<String> {
  values.iter().map(|value| String::from(*value)).collect()
}

#[test]
fn start_broadcasts_begin_hand_off() {
  let mut handoff = ShardCoordinatorHandoff::new();
  let (actions, outcome) = handoff.apply(ShardCoordinatorHandoffCommand::Start {
    shard_id:      String::from("10"),
    source_region: String::from("cluster://node-a"),
    regions:       regions(&["cluster://node-a", "cluster://node-b"]),
  });

  assert!(outcome.is_none());
  assert_eq!(actions, vec![ShardCoordinatorHandoffAction::SendBeginHandOff {
    shard_id: String::from("10"),
    regions:  regions(&["cluster://node-a", "cluster://node-b"]),
  }]);
  assert!(handoff.is_active());
}

#[test]
fn completes_after_all_acks_and_shard_stopped() {
  let mut handoff = ShardCoordinatorHandoff::new();
  let shard_id = String::from("10");
  let region_a = String::from("cluster://node-a");
  let region_b = String::from("cluster://node-b");

  let _ = handoff.apply(ShardCoordinatorHandoffCommand::Start {
    shard_id:      shard_id.clone(),
    source_region: region_a.clone(),
    regions:       regions(&["cluster://node-a", "cluster://node-b"]),
  });

  let (actions, outcome) = handoff
    .apply(ShardCoordinatorHandoffCommand::BeginHandOffAck { shard_id: shard_id.clone(), region: region_b.clone() });
  assert!(actions.is_empty());
  assert!(outcome.is_none());

  let (actions, outcome) = handoff
    .apply(ShardCoordinatorHandoffCommand::BeginHandOffAck { shard_id: shard_id.clone(), region: region_a.clone() });
  assert_eq!(actions, vec![ShardCoordinatorHandoffAction::SendHandOff {
    shard_id:      shard_id.clone(),
    source_region: region_a.clone(),
  }]);
  assert!(outcome.is_none());

  let (actions, outcome) = handoff.apply(ShardCoordinatorHandoffCommand::ShardStopped { shard_id: shard_id.clone() });
  assert!(actions.is_empty());
  assert_eq!(outcome, Some(ShardCoordinatorHandoffOutcome { shard_id, success: true }));
  assert!(!handoff.is_active());
}

#[test]
fn timeout_reports_failure() {
  let mut handoff = ShardCoordinatorHandoff::new();
  let shard_id = String::from("11");

  let _ = handoff.apply(ShardCoordinatorHandoffCommand::Start {
    shard_id:      shard_id.clone(),
    source_region: String::from("cluster://node-a"),
    regions:       regions(&["cluster://node-a"]),
  });

  let (actions, outcome) = handoff.apply(ShardCoordinatorHandoffCommand::Timeout);
  assert!(actions.is_empty());
  assert_eq!(outcome, Some(ShardCoordinatorHandoffOutcome { shard_id, success: false }));
}

#[test]
fn region_termination_during_stop_counts_as_success() {
  let mut handoff = ShardCoordinatorHandoff::new();
  let shard_id = String::from("12");
  let region_a = String::from("cluster://node-a");

  let _ = handoff.apply(ShardCoordinatorHandoffCommand::Start {
    shard_id:      shard_id.clone(),
    source_region: region_a.clone(),
    regions:       regions(&["cluster://node-a"]),
  });
  let _ = handoff
    .apply(ShardCoordinatorHandoffCommand::BeginHandOffAck { shard_id: shard_id.clone(), region: region_a.clone() });

  let (actions, outcome) = handoff.apply(ShardCoordinatorHandoffCommand::RegionTerminated { region: region_a });
  assert!(actions.is_empty());
  assert_eq!(outcome, Some(ShardCoordinatorHandoffOutcome { shard_id, success: true }));
}
