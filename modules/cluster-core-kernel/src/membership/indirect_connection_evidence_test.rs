use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  downing_provider::{DowningDecision, DowningInput, DowningProvider, NoopDowningProvider},
  membership::{ReachabilityMatrix, ReachabilityStatus},
};

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn partial_connectivity_distinguishes_direct_and_indirect_observations() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let subject = unique("subject-a", 3);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer_a.clone(), subject.clone());
  matrix.reachable(observer_b.clone(), subject.clone());

  let evidence = matrix.indirect_evidence_for(&subject).expect("partial connectivity evidence");

  assert_eq!(evidence.subject, subject.clone());
  assert_eq!(evidence.direct_observations.len(), 1);
  assert_eq!(evidence.direct_observations[0].observer, observer_a);
  assert_eq!(evidence.direct_observations[0].status, ReachabilityStatus::Unreachable);
  assert_eq!(evidence.indirect_observations.len(), 1);
  assert_eq!(evidence.indirect_observations[0].observer, observer_b);
  assert_eq!(evidence.indirect_observations[0].subject, subject);
  assert_eq!(evidence.indirect_observations[0].status, ReachabilityStatus::Reachable);
}

#[test]
fn observer_aggregate_statuses_are_included_in_evidence() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let observer_c = unique("observer-c", 3);
  let subject = unique("subject-a", 4);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer_a.clone(), subject.clone());
  matrix.reachable(observer_b.clone(), subject.clone());
  matrix.terminated(observer_c, observer_a.clone());

  let evidence = matrix.indirect_evidence_for(&subject).expect("partial connectivity evidence");

  let observer_a_status = evidence
    .observer_aggregate_statuses
    .iter()
    .find(|record| record.subject == observer_a)
    .expect("observer-a aggregate");
  let observer_b_status = evidence
    .observer_aggregate_statuses
    .iter()
    .find(|record| record.subject == observer_b)
    .expect("observer-b aggregate");
  assert_eq!(observer_a_status.status, ReachabilityStatus::Terminated);
  assert_eq!(observer_b_status.status, ReachabilityStatus::Reachable);
}

#[test]
fn direct_only_reachability_returns_no_indirect_evidence() {
  let observer = unique("observer-a", 1);
  let subject = unique("subject-a", 2);
  let mut matrix = ReachabilityMatrix::new();

  matrix.unreachable(observer, subject.clone());

  assert!(matrix.indirect_evidence_for(&subject).is_none());
}

#[test]
fn downing_input_carries_indirect_evidence_without_downing_decision() {
  let observer_a = unique("observer-a", 1);
  let observer_b = unique("observer-b", 2);
  let subject = unique("subject-a", 3);
  let mut matrix = ReachabilityMatrix::new();
  matrix.unreachable(observer_a, subject.clone());
  matrix.reachable(observer_b, subject.clone());
  let evidence = matrix.indirect_evidence_for(&subject).expect("partial connectivity evidence");
  let input = DowningInput::IndirectConnectionEvidence(evidence);
  let mut provider = NoopDowningProvider::new();

  assert_eq!(provider.decide(&input).unwrap(), DowningDecision::Defer);
}
