use alloc::vec;
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  downing_provider::{
    DowningDecision, DowningDecisionContext, DowningInput, DowningProvider, FailureObservation, FailureObservationKind,
    NoopDowningProvider,
  },
  membership::{IndirectConnectionEvidence, ReachabilityRecord, ReachabilityStatus},
};

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn noop_downing_provider_downs_explicit_command() {
  let mut provider = NoopDowningProvider::new();
  let input = DowningInput::explicit_down("node-a:2552");
  assert_eq!(provider.decide(&input).unwrap(), DowningDecision::Down);
}

#[test]
fn noop_downing_provider_defers_failure_observation() {
  let mut provider = NoopDowningProvider::new();
  let observation =
    FailureObservation::new("node-a:2552", FailureObservationKind::Suspect, TimerInstant::zero(Duration::from_secs(1)));
  let input = DowningInput::FailureObservation(observation);
  assert_eq!(provider.decide(&input).unwrap(), DowningDecision::Defer);
}

#[test]
fn noop_downing_provider_keeps_recovered_observation() {
  let mut provider = NoopDowningProvider::new();
  let observation = FailureObservation::new(
    "node-a:2552",
    FailureObservationKind::Recovered,
    TimerInstant::zero(Duration::from_secs(1)),
  );
  let input = DowningInput::FailureObservation(observation);
  assert_eq!(provider.decide(&input).unwrap(), DowningDecision::Keep);
}

#[test]
fn noop_downing_provider_decide_context_delegates_failure_observation() {
  let mut provider = NoopDowningProvider::new();
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));
  let input = DowningInput::FailureObservation(FailureObservation::new(
    "node-a:2552",
    FailureObservationKind::Recovered,
    evaluation_time,
  ));
  let context = DowningDecisionContext::from_downing_input(&input, evaluation_time);

  assert_eq!(provider.decide_context(&context).unwrap(), DowningDecision::Keep);
}

#[test]
fn noop_downing_provider_decide_context_delegates_indirect_evidence() {
  let mut provider = NoopDowningProvider::new();
  let observer = unique("observer", 1);
  let subject = unique("subject", 2);
  let evidence = IndirectConnectionEvidence {
    subject:                     subject.clone(),
    direct_observations:         vec![ReachabilityRecord {
      observer: observer.clone(),
      subject:  subject.clone(),
      status:   ReachabilityStatus::Unreachable,
      version:  1,
    }],
    indirect_observations:       vec![ReachabilityRecord {
      observer,
      subject: subject.clone(),
      status: ReachabilityStatus::Reachable,
      version: 2,
    }],
    observer_aggregate_statuses: vec![],
  };
  let evaluation_time = TimerInstant::zero(Duration::from_secs(1));
  let context =
    DowningDecisionContext::from_downing_input(&DowningInput::IndirectConnectionEvidence(evidence), evaluation_time);

  assert_eq!(provider.decide_context(&context).unwrap(), DowningDecision::Defer);
}
