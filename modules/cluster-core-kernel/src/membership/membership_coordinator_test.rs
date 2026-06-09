use alloc::{boxed::Box, string::String};
use core::time::Duration;

use fraktor_remote_core_rs::{
  address::{Address, UniqueAddress},
  failure_detector::PhiAccrualFailureDetector,
};
use fraktor_utils_core_rs::time::TimerInstant;

use super::MembershipCoordinator;
use crate::{
  ClusterEvent, ClusterExtensionConfig,
  failure_detector::{
    DefaultFailureDetectorRegistry, FailureDetector, FailureDetectorConfig, FailureDetectorConfigError,
  },
  membership::{
    DataCenter, MembershipCoordinatorConfig, MembershipCoordinatorError, MembershipCoordinatorState, MembershipDelta,
    MembershipError, MembershipEvent, MembershipTable, MembershipVersion, NodeRecord, NodeStatus, QuarantineEvent,
    ReachabilityStatus,
  },
  pub_sub::PubSubConfig,
};

/// Test-only adapter that bridges the remote-core detector to the
/// cluster-core `FailureDetector` trait.
struct PhiAccrualAdapter(PhiAccrualFailureDetector);

impl FailureDetector for PhiAccrualAdapter {
  fn is_available(&self, now_ms: u64) -> bool {
    self.0.is_available(now_ms)
  }

  fn is_monitoring(&self) -> bool {
    self.0.is_monitoring()
  }

  fn heartbeat(&mut self, now_ms: u64) {
    self.0.heartbeat(now_ms);
  }
}

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
    Box::new(PhiAccrualAdapter(PhiAccrualFailureDetector::new(detector_address(), threshold, 10, 1, 0, 10)))
  }))
}

fn detector_address() -> Address {
  Address::new("cluster-test", "127.0.0.1", 0)
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

fn local_cluster_config_with_address() -> ClusterExtensionConfig {
  local_cluster_config().with_advertised_address("local:2552")
}

#[test]
fn stopped_rejects_inputs() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_client().unwrap();

  let err =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap_err();
  assert_eq!(err, MembershipCoordinatorError::InvalidState { state: MembershipCoordinatorState::Client });

  let err = coordinator.handle_leave("node-a", now(1)).unwrap_err();
  assert_eq!(err, MembershipCoordinatorError::InvalidState { state: MembershipCoordinatorState::Client });
}

#[test]
fn start_member_rejects_invalid_failure_detector_config_before_running() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let cluster_config =
    local_cluster_config().with_failure_detector_config(FailureDetectorConfig::new().with_phi_threshold(0.0));
  let mut coordinator = MembershipCoordinator::new(config, cluster_config, table, registry(1.0));

  assert_eq!(
    coordinator.start_member().unwrap_err(),
    MembershipCoordinatorError::Configuration(FailureDetectorConfigError::InvalidPhiThreshold)
  );
  assert_eq!(coordinator.state(), MembershipCoordinatorState::Stopped);
}

#[test]
fn join_then_heartbeats_promote_through_weakly_up_to_up() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::WeaklyUp, .. }))
  );

  let outcome = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Up, .. }))
  );
}

#[test]
fn weakly_up_does_not_trigger_downing_decision() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let outcome = coordinator.handle_heartbeat("node-a", now(2)).unwrap();

  assert!(
    outcome
      .member_events
      .iter()
      .any(|event| { matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::WeaklyUp, .. }) })
  );
  assert!(
    !outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberQuarantined { .. } | ClusterEvent::UnreachableMember { .. }))
  );
  assert!(coordinator.quarantine_snapshot().is_empty());
}

#[test]
fn current_cluster_state_is_emitted_only_when_state_changes() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let join_outcome =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  assert!(join_outcome.member_events.iter().any(|event| matches!(event, ClusterEvent::CurrentClusterState { .. })));

  let weakly_up_outcome = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  assert!(
    weakly_up_outcome.member_events.iter().any(|event| matches!(event, ClusterEvent::CurrentClusterState { .. }))
  );

  let promote_outcome = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  assert!(promote_outcome.member_events.iter().any(|event| matches!(event, ClusterEvent::CurrentClusterState { .. })));

  let steady_outcome = coordinator.handle_heartbeat("node-a", now(4)).unwrap();
  assert!(steady_outcome.member_events.iter().all(|event| !matches!(event, ClusterEvent::CurrentClusterState { .. })));
}

#[test]
fn leave_emits_exiting_then_removed() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
fn suspect_timeout_keeps_observation_without_departure() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(1);
  config.topology_emit_interval = Duration::from_secs(10);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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
    !outcome
      .member_events
      .iter()
      .any(|event| matches!(event, ClusterEvent::MemberStatusChanged { to: NodeStatus::Dead, .. }))
  );
  assert!(!outcome.member_events.iter().any(|event| matches!(event, ClusterEvent::MemberQuarantined { .. })));
  assert!(coordinator.quarantine_snapshot().is_empty());
  let snapshot = coordinator.snapshot();
  assert!(
    snapshot.entries.iter().any(|record| { record.authority == "node-a" && record.status == NodeStatus::Suspect })
  );
}

#[test]
fn non_suspect_gossip_status_clears_suspect_tracking() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(1);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  let _ = coordinator.poll(now(5)).unwrap();
  assert!(coordinator.suspect_since.contains_key("node-a"));

  let dead_version = MembershipVersion::new(100);
  let dead_record = NodeRecord::new(
    "node-1".to_string(),
    "node-a".to_string(),
    NodeStatus::Dead,
    dead_version,
    "1.1.0".to_string(),
    vec![String::from("frontend")],
  );
  let dead_delta = MembershipDelta::new(MembershipVersion::new(99), dead_version, vec![dead_record]);
  let _ = coordinator.handle_gossip_delta("node-b", &dead_delta, now(6)).unwrap();
  assert!(!coordinator.suspect_since.contains_key("node-a"));
}

#[test]
fn local_leave_clears_suspect_tracking() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(1);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  let _ = coordinator.poll(now(5)).unwrap();
  assert!(coordinator.suspect_since.contains_key("node-a"));

  let _ = coordinator.handle_leave("node-a", now(6)).unwrap();
  assert!(!coordinator.suspect_since.contains_key("node-a"));
}

#[test]
fn join_rejects_incompatible_cluster_config() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let local = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(30)));
  let joining = ClusterExtensionConfig::new()
    .with_pubsub_config(PubSubConfig::new(Duration::from_secs(5), Duration::from_secs(30)));
  let mut coordinator = MembershipCoordinator::new(config, local, table, registry(1.0));
  coordinator.start_member().unwrap();

  let err = coordinator
    .handle_join("node-1".to_string(), "node-a".to_string(), &joining, now(1))
    .expect_err("incompatible join must be rejected");
  assert!(matches!(
    err,
    MembershipCoordinatorError::Membership(MembershipError::IncompatibleConfig { reason })
    if reason == "cluster.pubsub mismatch: pubsub configuration mismatch"
  ));
}

#[test]
fn join_rejects_invalid_joining_failure_detector_config_before_compatibility_check() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let joining =
    joining_cluster_config().with_failure_detector_config(FailureDetectorConfig::new().with_max_sample_size(0));

  let err = coordinator
    .handle_join("node-1".to_string(), "node-a".to_string(), &joining, now(1))
    .expect_err("invalid joining failure detector config must be rejected");
  assert_eq!(err, MembershipCoordinatorError::Configuration(FailureDetectorConfigError::ZeroMaxSampleSize));
}

#[test]
fn join_uses_joining_config_metadata() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
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

#[test]
fn current_cluster_state_emits_oldest_leader_and_role_leaders() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let joining_backend = ClusterExtensionConfig::new()
    .with_app_version("1.0.0")
    .with_roles(vec![String::from("backend"), String::from("shared")]);
  let joining_frontend = ClusterExtensionConfig::new()
    .with_app_version("1.0.0")
    .with_roles(vec![String::from("frontend"), String::from("shared")]);

  let _ = coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_backend, now(1)).unwrap();
  let _ = coordinator.handle_join("node-2".to_string(), "node-b".to_string(), &joining_frontend, now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(4)).unwrap();
  let _ = coordinator.handle_heartbeat("node-b", now(5)).unwrap();
  let outcome = coordinator.handle_heartbeat("node-b", now(6)).unwrap();

  let state = outcome
    .member_events
    .iter()
    .find_map(
      |event| {
        if let ClusterEvent::CurrentClusterState { state, .. } = event { Some(state.clone()) } else { None }
      },
    )
    .expect("current cluster state");

  assert_eq!(state.leader, Some(String::from("node-a")));
  assert_eq!(state.role_leader.get("backend"), Some(&Some(String::from("node-a"))));
  assert_eq!(state.role_leader.get("frontend"), Some(&Some(String::from("node-b"))));
  assert_eq!(state.role_leader.get("shared"), Some(&Some(String::from("node-a"))));
}

#[test]
fn current_cluster_state_keeps_roles_without_eligible_leader_as_none() {
  let table = MembershipTable::new(3);
  let config = base_config();
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let joining_backend =
    ClusterExtensionConfig::new().with_app_version("1.0.0").with_roles(vec![String::from("backend")]);
  let joining_analytics =
    ClusterExtensionConfig::new().with_app_version("1.0.0").with_roles(vec![String::from("analytics")]);

  let _ = coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_backend, now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  let outcome =
    coordinator.handle_join("node-2".to_string(), "node-b".to_string(), &joining_analytics, now(4)).unwrap();

  let state = outcome
    .member_events
    .iter()
    .find_map(
      |event| {
        if let ClusterEvent::CurrentClusterState { state, .. } = event { Some(state.clone()) } else { None }
      },
    )
    .expect("current cluster state");

  assert_eq!(state.role_leader.get("backend"), Some(&Some(String::from("node-a"))));
  assert_eq!(state.role_leader.get("analytics"), Some(&None));
}

#[test]
fn current_cluster_state_does_not_use_suspect_oldest_for_leader() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(1);
  config.dead_timeout = Duration::from_secs(30);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let role = ClusterExtensionConfig::new().with_app_version("1.0.0").with_roles(vec![String::from("backend")]);
  let _ = coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &role, now(1)).unwrap();
  let _ = coordinator.handle_join("node-2".to_string(), "node-b".to_string(), &role, now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(4)).unwrap();
  let _ = coordinator.handle_heartbeat("node-b", now(5)).unwrap();
  let _ = coordinator.handle_heartbeat("node-b", now(6)).unwrap();

  let _ = coordinator.poll(now(7)).unwrap();
  let outcome = coordinator.handle_heartbeat("node-b", now(8)).unwrap();
  let state = outcome
    .member_events
    .iter()
    .find_map(
      |event| {
        if let ClusterEvent::CurrentClusterState { state, .. } = event { Some(state.clone()) } else { None }
      },
    )
    .expect("current cluster state");

  assert!(state.unreachable.iter().any(|record| record.authority == "node-a"));
  assert_eq!(state.leader, Some(String::from("node-b")));
  assert_eq!(state.role_leader.get("backend"), Some(&Some(String::from("node-b"))));
}

#[test]
fn suspect_and_heartbeat_emit_unreachable_and_reachable_events() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(30);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();

  let suspect_outcome = coordinator.poll(now(5)).unwrap();
  assert!(suspect_outcome.member_events.iter().any(|event| matches!(
    event,
    ClusterEvent::UnreachableMember { authority, .. } if authority == "node-a"
  )));

  let reachable_outcome = coordinator.handle_heartbeat("node-a", now(6)).unwrap();
  assert!(reachable_outcome.member_events.iter().any(|event| matches!(
    event,
    ClusterEvent::ReachableMember { authority, .. } if authority == "node-a"
  )));
}

#[test]
fn reachability_snapshot_tracks_failure_detector_and_heartbeat_receipt() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(30);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config_with_address(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();

  let suspect_outcome = coordinator.poll(now(5)).unwrap();
  let subject = coordinator.snapshot().entries[0].unique_address.clone();
  let state = suspect_outcome
    .member_events
    .iter()
    .find_map(
      |event| {
        if let ClusterEvent::CurrentClusterState { state, .. } = event { Some(state.clone()) } else { None }
      },
    )
    .expect("current cluster state");
  assert_eq!(coordinator.snapshot().reachability.aggregate_status(&subject), ReachabilityStatus::Unreachable);
  assert_eq!(state.reachability.aggregate_status(&subject), ReachabilityStatus::Unreachable);

  let _ = coordinator.handle_heartbeat("node-a", now(6)).unwrap();

  assert_eq!(coordinator.snapshot().reachability.aggregate_status(&subject), ReachabilityStatus::Reachable);
}

#[test]
fn reachability_snapshot_tracks_suspect_without_advertised_address() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.suspect_timeout = Duration::from_secs(30);
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let _ =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(2)).unwrap();
  let _ = coordinator.handle_heartbeat("node-a", now(3)).unwrap();

  let _ = coordinator.poll(now(5)).unwrap();
  let subject = coordinator.snapshot().entries[0].unique_address.clone();

  assert_eq!(coordinator.snapshot().reachability.aggregate_status(&subject), ReachabilityStatus::Unreachable);
}

#[test]
fn gossip_delta_new_incarnation_removes_previous_active_from_current_state() {
  let mut table = MembershipTable::new(3);
  let address = Address::new("cluster", "node-a", 2552);
  let first = UniqueAddress::new(address.clone(), 10);
  let second = UniqueAddress::new(address, 11);
  table
    .try_join_with_identity("node-1".to_string(), first.clone(), DataCenter::default(), "1.0.0".to_string(), vec![])
    .expect("first incarnation joins");
  table.mark_weakly_up("cluster@node-a:2552").expect("first weakly up").expect("delta");
  table.mark_up("cluster@node-a:2552").expect("first up").expect("delta");
  let version = table.version();
  let second_record = NodeRecord::new_with_identity(
    second.clone(),
    DataCenter::default(),
    "node-1".to_string(),
    NodeStatus::Joining,
    version.next(),
    "1.0.1".to_string(),
    vec![],
  );
  let delta = MembershipDelta::new(version, version.next(), vec![second_record]);

  let mut coordinator = MembershipCoordinator::new(base_config(), local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let outcome = coordinator.handle_gossip_delta("peer", &delta, now(6)).unwrap();
  let state = outcome
    .member_events
    .iter()
    .find_map(
      |event| {
        if let ClusterEvent::CurrentClusterState { state, .. } = event { Some(state.clone()) } else { None }
      },
    )
    .expect("current cluster state");

  assert!(!state.members.iter().any(|record| record.unique_address == first));
  assert!(state.members.iter().any(|record| record.unique_address == second));
  assert!(outcome.member_events.iter().any(|event| matches!(
    event,
    ClusterEvent::MemberStatusChanged { authority, from: NodeStatus::Up, to: NodeStatus::Dead, .. }
      if authority == "cluster@node-a:2552"
  )));
  assert!(outcome.member_events.iter().any(|event| matches!(
    event,
    ClusterEvent::MemberQuarantined { authority, reason, .. }
      if authority == "cluster@node-a:2552" && reason == "gossip-dead"
  )));
}

#[test]
fn gossip_seen_changed_event_is_emitted() {
  let table = MembershipTable::new(3);
  let mut config = base_config();
  config.gossip_enabled = true;
  let mut coordinator = MembershipCoordinator::new(config, local_cluster_config(), table, registry(1.0));
  coordinator.start_member().unwrap();

  let outcome =
    coordinator.handle_join("node-1".to_string(), "node-a".to_string(), &joining_cluster_config(), now(1)).unwrap();
  assert!(outcome.member_events.iter().any(|event| matches!(
    event,
    ClusterEvent::SeenChanged { version, .. } if *version == MembershipVersion::new(1)
  )));
}
