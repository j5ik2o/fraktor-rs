use alloc::{boxed::Box, string::String};
use core::time::Duration;

use fraktor_remote_rs::core::failure_detector::{
  DefaultFailureDetectorRegistry,
  phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig},
};
use fraktor_utils_rs::core::time::TimerInstant;

use super::MembershipCoordinatorGeneric;
use crate::core::{
  ClusterEvent, ClusterExtensionConfig,
  membership::{
    MembershipCoordinatorConfig, MembershipCoordinatorError, MembershipCoordinatorState, MembershipDelta,
    MembershipError, MembershipEvent, MembershipTable, MembershipVersion, NodeStatus, QuarantineEvent,
  },
  pub_sub::PubSubConfig,
};

fn base_config() -> MembershipCoordinatorConfig {
  MembershipCoordinatorConfig {
    phi_threshold:          1.0,
    suspect_timeout:        Duration::from_secs(1),
    dead_timeout:           Duration::from_secs(0),
    quarantine_ttl:         Duration::from_secs(1),
    gossip_enabled:         false,
    gossip_interval:        Duration::from_secs(1),
    topology_emit_interval: Duration::from_secs(1),
  }
}

fn registry(threshold: f64) -> DefaultFailureDetectorRegistry<String> {
  DefaultFailureDetectorRegistry::new(Box::new(move || {
    Box::new(PhiFailureDetector::new(PhiFailureDetectorConfig::new(threshold, 10, 1)))
  }))
}

fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

fn local_cluster_config() -> ClusterExtensionConfig {
  ClusterExtensionConfig::new().with_app_version("1.0.0").with_roles(vec![String::from("backend")])
}

fn joining_cluster_config() -> ClusterExtensionConfig {
  ClusterExtensionConfig::new().with_app_version("1.1.0").with_roles(vec![String::from("frontend")])
}

#[test]
fn stopped_rejects_inputs() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  let now = now(1);

  assert_eq!(coordinator.handle_heartbeat("node-a", now).unwrap_err(), MembershipCoordinatorError::NotStarted);
  assert_eq!(coordinator.poll(now).unwrap_err(), MembershipCoordinatorError::NotStarted);

  let delta = MembershipDelta::new(MembershipVersion::zero(), MembershipVersion::zero(), Vec::new());
  assert_eq!(coordinator.handle_gossip_delta("peer", &delta, now).unwrap_err(), MembershipCoordinatorError::NotStarted);
}

#[test]
fn client_rejects_join_and_leave() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_client().unwrap();

  let err =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap_err();
  assert_eq!(err, MembershipCoordinatorError::InvalidState { state: MembershipCoordinatorState::Client });

  let err = coordinator.handle_leave("node-a", now(1)).unwrap_err();
  assert_eq!(err, MembershipCoordinatorError::InvalidState { state: MembershipCoordinatorState::Client });
}

#[test]
fn join_then_heartbeat_promotes_to_up() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let outcome =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Joining, .. }))
  );

  let outcome = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Up, .. }))
  );
}

#[test]
fn leave_emits_exiting_then_removed() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();

  let outcome = coordinator.handle_leave("node-a", now(3)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Exiting, .. }))
  );
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Removed, .. }))
  );
  assert!(
    outcome
      .membership_events
      .iter()
      .any(|event| matches!(event, MembershipEvent::Left { authority, .. } if authority == "node-a"))
  );
}

#[test]
fn topology_emits_after_interval() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.topology_emit_interval = Duration::from_secs(2);
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();

  let outcome = coordinator.poll(now(1)).unwrap();
  assert!(outcome.topology_event.is_none());

  let outcome = coordinator.poll(now(3)).unwrap();
  assert!(outcome.topology_event.is_some());
}

#[test]
fn quarantine_rejects_join_and_expires() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.quarantine_ttl = Duration::from_secs(1);
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let outcome = coordinator.handle_quarantine("node-a".to_string(), "manual".to_string(), now(1)).unwrap();
  assert!(outcome.quarantine_events.iter().any(|event| matches!(event, QuarantineEvent::Quarantined { .. })));

  let err =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap_err();
  assert!(matches!(err, MembershipCoordinatorError::Membership(MembershipError::Quarantined { .. })));

  let outcome = coordinator.poll(now(3)).unwrap();
  assert!(outcome.quarantine_events.iter().any(|event| matches!(event, QuarantineEvent::Cleared { .. })));
}

#[test]
fn suspect_timeout_marks_dead_and_quarantines() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(1);
  config.topology_emit_interval = Duration::from_secs(10);
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();

  let outcome = coordinator.poll(now(5)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Suspect, .. }))
  );

  let outcome = coordinator.poll(now(7)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Dead, .. }))
  );
  assert!(outcome.member_events.iter().any(|event| matches!(event, ClusterEvent::MemberQuarantined { .. })));
}

#[test]
fn join_rejects_incompatible_cluster_config() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(30)));
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(5), Duration::from_secs(30)));
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local,
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let err = coordinator
    .handle_join("node-1".to_string(), "node-a".to_string(), &joining, now(1))
    .expect_err("incompatible join must be rejected");
  assert!(matches!(
    err,
    MembershipCoordinatorError::Membership(MembershipError::IncompatibleConfig { reason })
    if reason == "pubsub configuration mismatch"
  ));
}

#[test]
fn join_uses_joining_config_metadata() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinatorGeneric::<fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox>::new(
    config,
    local_cluster_config(),
    table,
    registry(1.0),
  );
  coordinator.start_member().unwrap();

  let joining = ClusterExtensionConfig::new()
    .with_app_version("2.0.0")
    .with_roles(vec![String::from("edge"), String::from("frontend")]);
  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining, now(1)).expect("join should succeed");

  let snapshot = coordinator.snapshot();
  assert_eq!(snapshot.entries.len(), 1);
  assert_eq!(snapshot.entries[0].app_version, "2.0.0");
  assert_eq!(snapshot.entries[0].roles, vec![String::from("edge"), String::from("frontend")]);
}
