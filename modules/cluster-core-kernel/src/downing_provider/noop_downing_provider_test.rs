use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use crate::downing_provider::{
  DowningDecision, DowningInput, DowningProvider, FailureObservation, FailureObservationKind, NoopDowningProvider,
};

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
