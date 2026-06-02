use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  downing_provider::{
    DowningDecision, DowningDecisionContext, DowningInput, DowningProvider, LeaseAcquisitionOutcome,
    SplitBrainResolverSettings, SplitBrainResolverStrategy,
  },
  extension::ClusterProviderError,
  membership::{DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix},
};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

use super::{StdLeaseMajorityBackend, StdSplitBrainResolverProvider};

#[derive(Clone)]
struct RecordingLeaseBackend {
  outcome: LeaseAcquisitionOutcome,
  calls:   ArcShared<SpinSyncMutex<usize>>,
  closed:  ArcShared<SpinSyncMutex<usize>>,
}

impl RecordingLeaseBackend {
  fn new(
    outcome: LeaseAcquisitionOutcome,
    calls: ArcShared<SpinSyncMutex<usize>>,
    closed: ArcShared<SpinSyncMutex<usize>>,
  ) -> Self {
    Self { outcome, calls, closed }
  }
}

impl StdLeaseMajorityBackend for RecordingLeaseBackend {
  fn acquire(&mut self, _context: &DowningDecisionContext) -> LeaseAcquisitionOutcome {
    *self.calls.lock() += 1;
    self.outcome
  }

  fn close(&mut self) {
    *self.closed.lock() += 1;
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

fn lease_majority_settings() -> SplitBrainResolverSettings {
  SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30))
}

#[test]
fn start_constructs_hook_and_lease_backend_adapter() {
  let calls = ArcShared::new(SpinSyncMutex::new(0));
  let closed = ArcShared::new(SpinSyncMutex::new(0));
  let calls_for_factory = calls.clone();
  let closed_for_factory = closed.clone();
  let mut provider =
    StdSplitBrainResolverProvider::new(lease_majority_settings()).with_lease_backend_factory(move || {
      Box::new(RecordingLeaseBackend::new(
        LeaseAcquisitionOutcome::Acquired,
        calls_for_factory.clone(),
        closed_for_factory.clone(),
      ))
    });

  provider.start().expect("provider starts");
  let decision = provider.decide_context(&majority_context());

  assert_eq!(decision, Ok(DowningDecision::Keep));
  assert_eq!(*calls.lock(), 1);
  assert_eq!(*closed.lock(), 0);
}

#[test]
fn stop_closes_active_backend_and_rejects_decisions() {
  let calls = ArcShared::new(SpinSyncMutex::new(0));
  let closed = ArcShared::new(SpinSyncMutex::new(0));
  let calls_for_factory = calls.clone();
  let closed_for_factory = closed.clone();
  let mut provider =
    StdSplitBrainResolverProvider::new(lease_majority_settings()).with_lease_backend_factory(move || {
      Box::new(RecordingLeaseBackend::new(
        LeaseAcquisitionOutcome::Acquired,
        calls_for_factory.clone(),
        closed_for_factory.clone(),
      ))
    });

  provider.start().expect("provider starts");
  provider.stop().expect("provider stops");
  let err = provider.decide_context(&majority_context()).expect_err("provider is stopped");

  assert!(matches!(err, ClusterProviderError::DownFailed(_)));
  assert_eq!(err.reason(), "split-brain-resolver provider is not started");
  assert_eq!(*calls.lock(), 0);
  assert_eq!(*closed.lock(), 1);
}

#[test]
fn drop_closes_active_backend() {
  let calls = ArcShared::new(SpinSyncMutex::new(0));
  let closed = ArcShared::new(SpinSyncMutex::new(0));
  {
    let calls_for_factory = calls.clone();
    let closed_for_factory = closed.clone();
    let mut provider =
      StdSplitBrainResolverProvider::new(lease_majority_settings()).with_lease_backend_factory(move || {
        Box::new(RecordingLeaseBackend::new(
          LeaseAcquisitionOutcome::Acquired,
          calls_for_factory.clone(),
          closed_for_factory.clone(),
        ))
      });
    provider.start().expect("provider starts");
  }

  assert_eq!(*calls.lock(), 0);
  assert_eq!(*closed.lock(), 1);
}

#[test]
fn downing_provider_decide_delegates_to_core_hook() {
  let mut provider = StdSplitBrainResolverProvider::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));
  provider.start().expect("provider starts");

  let decision = provider.decide(&DowningInput::explicit_down("node-a:2552"));

  assert_eq!(decision, Ok(DowningDecision::Down));
}
