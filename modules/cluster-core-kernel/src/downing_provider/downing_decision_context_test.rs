use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  downing_provider::{
    DowningDecision, DowningDecisionContext, DowningInput, DowningProvider, FailureObservation, FailureObservationKind,
    NoopDowningProvider,
  },
  membership::{
    DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix, ReachabilityStatus,
  },
};

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn node_record(identity: UniqueAddress, data_center: DataCenter, node_id: &str, status: NodeStatus) -> NodeRecord {
  NodeRecord::new_with_identity(
    identity,
    data_center,
    String::from(node_id),
    status,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  )
}

#[test]
fn membership_context_preserves_member_evidence_and_evaluation_time() {
  let observer = unique("observer", 1);
  let indirect_observer = unique("observer-b", 2);
  let weakly_up_member = unique("weakly-up", 3);
  let terminated_member = unique("terminated", 4);
  let east = DataCenter::new("dc-east");
  let west = DataCenter::new("dc-west");
  let weakly_up_record = node_record(weakly_up_member.clone(), east.clone(), "node-weakly-up", NodeStatus::WeaklyUp);
  let terminated_record = node_record(terminated_member.clone(), west.clone(), "node-terminated", NodeStatus::Dead);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(observer.clone(), weakly_up_member.clone());
  reachability.reachable(indirect_observer, weakly_up_member.clone());
  reachability.terminated(observer, terminated_member.clone());
  let indirect_evidence =
    reachability.indirect_evidence_for(&weakly_up_member).expect("indirect reachability evidence");
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(2),
    vec![weakly_up_record, terminated_record],
    reachability.snapshot(),
  );
  let evaluation_time = TimerInstant::zero(Duration::from_millis(100));

  let context = DowningDecisionContext::from_membership_snapshot_with_indirect_evidence(
    snapshot,
    indirect_evidence,
    evaluation_time,
  );

  assert_eq!(context.evaluation_time(), evaluation_time);
  assert_eq!(context.indirect_connection_evidence().expect("indirect evidence").subject, weakly_up_member);
  assert_eq!(context.member_record(&weakly_up_member).expect("weakly-up member").status, NodeStatus::WeaklyUp);
  assert_eq!(context.member_record(&weakly_up_member).expect("weakly-up member").data_center, east);
  assert_eq!(context.reachability_status(&weakly_up_member), Some(ReachabilityStatus::Unreachable));
  assert_eq!(context.member_record(&terminated_member).expect("terminated member").data_center, west);
  assert_eq!(context.reachability_status(&terminated_member), Some(ReachabilityStatus::Terminated));
  assert_eq!(context.defer_reason(), None);
}

#[test]
fn explicit_down_context_does_not_require_membership_evidence() {
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));

  let context = DowningDecisionContext::from_explicit_down("node-a:2552", evaluation_time);

  assert_eq!(context.evaluation_time(), evaluation_time);
  assert_eq!(context.explicit_down_authority(), Some("node-a:2552"));
  assert_eq!(context.membership_snapshot(), None);
  assert_eq!(context.indirect_connection_evidence(), None);
  assert!(!context.requires_reachability_evidence());
  assert_eq!(context.defer_reason(), None);
}

#[test]
fn missing_reachability_evidence_generates_defer_reason() {
  let subject = unique("subject", 1);
  let snapshot = MembershipSnapshot::new(MembershipVersion::new(1), vec![node_record(
    subject,
    DataCenter::new("dc-east"),
    "node-a",
    NodeStatus::Up,
  )]);

  let context =
    DowningDecisionContext::from_membership_snapshot(snapshot, TimerInstant::zero(Duration::from_millis(100)));

  assert!(context.requires_reachability_evidence());
  assert_eq!(context.defer_reason(), Some("reachability evidence is required for membership evaluation"));
}

#[test]
fn reachable_observer_version_counts_as_reachability_evidence() {
  let observer = unique("observer", 1);
  let subject = unique("subject", 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.reachable(observer, subject.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(1),
    vec![node_record(subject, DataCenter::new("dc-east"), "node-a", NodeStatus::Up)],
    reachability.snapshot(),
  );

  let context =
    DowningDecisionContext::from_membership_snapshot(snapshot, TimerInstant::zero(Duration::from_millis(100)));

  assert!(!context.requires_reachability_evidence());
  assert_eq!(context.defer_reason(), None);
}

#[test]
fn downing_input_explicit_down_creates_context_without_membership_snapshot() {
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));
  let input = DowningInput::explicit_down("node-a:2552");

  let context = DowningDecisionContext::from_downing_input(&input, evaluation_time);

  assert_eq!(context.evaluation_time(), evaluation_time);
  assert_eq!(context.explicit_down_authority(), Some("node-a:2552"));
  assert_eq!(context.membership_snapshot(), None);
  assert_eq!(context.failure_observation(), None);
  assert!(!context.requires_reachability_evidence());
}

#[test]
fn failure_observation_context_can_attach_membership_reachability_snapshot() {
  let subject = unique("subject", 1);
  let observer = unique("observer", 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(observer, subject.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(1),
    vec![node_record(subject.clone(), DataCenter::new("dc-east"), "node-a", NodeStatus::WeaklyUp)],
    reachability.snapshot(),
  );
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));
  let observation = FailureObservation::new("node-a", FailureObservationKind::Unreachable, evaluation_time);
  let input = DowningInput::FailureObservation(observation.clone());

  let context = DowningDecisionContext::from_downing_input_with_membership_snapshot(&input, snapshot, evaluation_time);

  assert_eq!(context.failure_observation(), Some(&observation));
  assert_eq!(context.member_record(&subject).expect("subject").status, NodeStatus::WeaklyUp);
  assert_eq!(context.reachability_status(&subject), Some(ReachabilityStatus::Unreachable));
  assert_eq!(context.defer_reason(), None);
}

#[test]
fn noop_provider_behavior_is_unchanged_after_context_conversion() {
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));
  let input = DowningInput::FailureObservation(FailureObservation::new(
    "node-a:2552",
    FailureObservationKind::Recovered,
    evaluation_time,
  ));
  let context = DowningDecisionContext::from_downing_input(&input, evaluation_time);
  let mut provider = NoopDowningProvider::new();

  assert_eq!(context.failure_observation().expect("observation").kind(), FailureObservationKind::Recovered);
  assert_eq!(provider.decide(&input).unwrap(), DowningDecision::Keep);
}
