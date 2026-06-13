use alloc::{string::String, vec, vec::Vec};

use super::GrainReadinessSnapshot;
use crate::{
  activation::PlacementCoordinatorState,
  grain::{GrainReadiness, GrainUnreadyReason},
  membership::NodeStatus,
};

fn kinds(names: &[&str]) -> Vec<String> {
  names.iter().map(|n| String::from(*n)).collect()
}

#[test]
fn ready_when_up_resolvable_and_all_kinds_registered() {
  let snapshot =
    GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::Member, kinds(&["worker", "ledger"]));

  assert_eq!(snapshot.readiness(&kinds(&["worker", "ledger"])), GrainReadiness::Ready);
}

#[test]
fn weakly_up_is_accepted_as_up() {
  let snapshot =
    GrainReadinessSnapshot::new(Some(NodeStatus::WeaklyUp), PlacementCoordinatorState::Member, kinds(&["worker"]));

  assert_eq!(snapshot.readiness(&kinds(&["worker"])), GrainReadiness::Ready);
}

#[test]
fn client_placement_is_resolvable() {
  let snapshot = GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::Client, Vec::new());

  assert_eq!(snapshot.readiness(&[]), GrainReadiness::Ready);
}

#[test]
fn not_ready_reports_placement_not_ready() {
  let snapshot =
    GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::NotReady, kinds(&["worker"]));

  assert_eq!(snapshot.readiness(&kinds(&["worker"])), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::PlacementNotReady { state: PlacementCoordinatorState::NotReady }],
  });
}

#[test]
fn stopped_placement_is_not_resolvable() {
  let snapshot = GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::Stopped, Vec::new());

  assert_eq!(snapshot.readiness(&[]), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::PlacementNotReady { state: PlacementCoordinatorState::Stopped }],
  });
}

#[test]
fn not_ready_when_self_node_absent() {
  let snapshot = GrainReadinessSnapshot::new(None, PlacementCoordinatorState::Member, Vec::new());

  assert_eq!(snapshot.readiness(&[]), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: None }],
  });
}

#[test]
fn joining_self_node_is_not_up() {
  let snapshot = GrainReadinessSnapshot::new(Some(NodeStatus::Joining), PlacementCoordinatorState::Member, Vec::new());

  assert_eq!(snapshot.readiness(&[]), GrainReadiness::NotReady {
    reasons: vec![GrainUnreadyReason::SelfNodeNotUp { status: Some(NodeStatus::Joining) }],
  });
}

#[test]
fn not_ready_reports_each_unregistered_kind() {
  let snapshot =
    GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::Member, kinds(&["worker"]));

  assert_eq!(snapshot.readiness(&kinds(&["worker", "ledger", "billing"])), GrainReadiness::NotReady {
    reasons: vec![
      GrainUnreadyReason::KindNotRegistered { kind: String::from("ledger") },
      GrainUnreadyReason::KindNotRegistered { kind: String::from("billing") },
    ],
  });
}

#[test]
fn empty_expected_kinds_does_not_impose_kind_condition() {
  let snapshot = GrainReadinessSnapshot::new(Some(NodeStatus::Up), PlacementCoordinatorState::Member, Vec::new());

  assert_eq!(snapshot.readiness(&[]), GrainReadiness::Ready);
}

#[test]
fn collects_all_unmet_conditions_in_self_placement_kind_order() {
  let snapshot = GrainReadinessSnapshot::new(None, PlacementCoordinatorState::NotReady, Vec::new());

  assert_eq!(snapshot.readiness(&kinds(&["worker"])), GrainReadiness::NotReady {
    reasons: vec![
      GrainUnreadyReason::SelfNodeNotUp { status: None },
      GrainUnreadyReason::PlacementNotReady { state: PlacementCoordinatorState::NotReady },
      GrainUnreadyReason::KindNotRegistered { kind: String::from("worker") },
    ],
  });
}

#[test]
fn same_input_yields_same_result() {
  let snapshot =
    GrainReadinessSnapshot::new(Some(NodeStatus::Joining), PlacementCoordinatorState::Stopped, kinds(&["worker"]));
  let expected = kinds(&["worker", "ledger"]);

  assert_eq!(snapshot.readiness(&expected), snapshot.readiness(&expected));
}
