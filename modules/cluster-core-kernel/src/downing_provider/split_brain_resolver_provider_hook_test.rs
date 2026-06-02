use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  ClusterProviderError,
  downing_provider::{
    DowningDecision, DowningDecisionContext, DowningInput, DowningProvider, DowningProviderCompatibility,
    LeaseAcquisitionOutcome, LeaseMajorityPort, SplitBrainResolverProviderHook, SplitBrainResolverSettings,
    SplitBrainResolverStrategy,
  },
  membership::{DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix},
};

struct RecordingLeasePort {
  outcome: LeaseAcquisitionOutcome,
  calls:   usize,
}

impl LeaseMajorityPort for RecordingLeasePort {
  fn acquire_majority(&mut self, _context: &DowningDecisionContext) -> LeaseAcquisitionOutcome {
    self.calls += 1;
    self.outcome
  }
}

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn record(host: &str, uid: u64, status: NodeStatus, join_version: u64) -> NodeRecord {
  NodeRecord::new_with_identity(
    unique(host, uid),
    DataCenter::default(),
    String::from(host),
    status,
    MembershipVersion::new(join_version),
    String::from("1.0.0"),
    vec![String::from("backend")],
  )
}

fn majority_context() -> DowningDecisionContext {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b, node_c],
    reachability.snapshot(),
  );
  DowningDecisionContext::from_membership_snapshot(snapshot, TimerInstant::zero(Duration::from_millis(1)))
}

#[test]
fn provider_hook_exposes_sbr_compatibility_metadata() {
  let settings = SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  );
  let hook = SplitBrainResolverProviderHook::new(settings);

  let compatibility = hook.compatibility();

  assert_eq!(compatibility.provider_key(), "split-brain-resolver");
  assert_eq!(compatibility.split_brain_resolver_settings(), Some(&settings));
  assert_eq!(
    compatibility.sbr_settings_identity(),
    Some("stable-after-nanos=20000000000;active-strategy=keep-majority;down-all-when-unstable-nanos=30000000000"),
  );
}

#[test]
fn provider_hook_rejects_mismatched_metadata() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepMajority, Duration::from_secs(30));
  let compatibility = DowningProviderCompatibility::new("split-brain-resolver").with_split_brain_resolver_settings(
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepOldest, Duration::from_secs(30)),
  );

  let err = SplitBrainResolverProviderHook::from_compatibility(settings, compatibility).expect_err("metadata mismatch");

  assert!(matches!(err, ClusterProviderError::DownFailed(_)));
  assert!(err.reason().contains("split-brain-resolver compatibility metadata mismatch"));
}

#[test]
fn provider_hook_maps_explicit_down_without_membership_snapshot() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);

  let decision = hook.decide(&DowningInput::explicit_down("node-a:2552"));

  assert_eq!(decision, Ok(DowningDecision::Down));
}

#[test]
fn provider_hook_downing_provider_decide_context_uses_membership_context() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::KeepMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);
  let downing_provider: &mut dyn DowningProvider = &mut hook;

  let decision = downing_provider.decide_context(&majority_context());

  assert_eq!(decision, Ok(DowningDecision::Keep));
}

#[test]
fn provider_hook_prioritizes_explicit_down_before_lease_backend_failure() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);

  let decision = hook.decide(&DowningInput::explicit_down("node-a:2552"));

  assert_eq!(decision, Ok(DowningDecision::Down));
}

#[test]
fn provider_hook_maps_decision_failure_to_cluster_provider_error() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);

  let err = hook.decide_context(&majority_context()).expect_err("missing lease backend");

  assert!(matches!(err, ClusterProviderError::DownFailed(_)));
  assert_eq!(err.reason(), "lease backend missing");
}

#[test]
fn provider_hook_routes_context_to_lease_port() {
  let settings =
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30));
  let mut hook = SplitBrainResolverProviderHook::new(settings);
  let mut lease_port = RecordingLeasePort { outcome: LeaseAcquisitionOutcome::Acquired, calls: 0 };

  let decision = hook.decide_context_with_lease(&majority_context(), &mut lease_port);

  assert_eq!(decision, Ok(DowningDecision::Keep));
  assert_eq!(lease_port.calls, 1);
}
