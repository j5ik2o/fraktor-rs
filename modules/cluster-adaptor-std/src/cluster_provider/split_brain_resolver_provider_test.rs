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
  sync::{DefaultMutex, SharedAccess, SharedLock},
  time::TimerInstant,
};

use super::{StdLeaseMajorityBackend, StdSplitBrainResolverProvider};

#[derive(Clone)]
struct RecordingLeaseBackend {
  outcome: LeaseAcquisitionOutcome,
  calls:   SharedLock<usize>,
  closed:  SharedLock<usize>,
}

impl RecordingLeaseBackend {
  fn new(outcome: LeaseAcquisitionOutcome, calls: SharedLock<usize>, closed: SharedLock<usize>) -> Self {
    Self { outcome, calls, closed }
  }
}

impl StdLeaseMajorityBackend for RecordingLeaseBackend {
  fn acquire(&mut self, _context: &DowningDecisionContext) -> LeaseAcquisitionOutcome {
    self.calls.with_write(|calls| *calls += 1);
    self.outcome
  }

  fn close(&mut self) {
    self.closed.with_write(|closed| *closed += 1);
  }
}

fn counter() -> SharedLock<usize> {
  SharedLock::new_with_driver::<DefaultMutex<_>>(0)
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

fn majority_snapshot() -> MembershipSnapshot {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b, node_c],
    reachability.snapshot(),
  )
}

fn majority_context() -> DowningDecisionContext {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  DowningDecisionContext::from_membership_snapshot(majority_snapshot(), TimerInstant::zero(Duration::from_millis(1)))
    .with_reachability_observer(node_a.unique_address)
}

fn majority_context_at(evaluation_time: TimerInstant, unstable_since: TimerInstant) -> DowningDecisionContext {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  DowningDecisionContext::from_membership_snapshot(majority_snapshot(), evaluation_time)
    .with_reachability_observer(node_a.unique_address)
    .with_unstable_since(unstable_since)
}

fn multi_observer_context_with_local_observer() -> DowningDecisionContext {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  reachability.reachable(node_b.unique_address.clone(), node_c.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b, node_c],
    reachability.snapshot(),
  );
  DowningDecisionContext::from_membership_snapshot(snapshot, TimerInstant::zero(Duration::from_millis(1)))
    .with_reachability_observer(node_a.unique_address)
}

fn lease_majority_settings() -> SplitBrainResolverSettings {
  SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::LeaseMajority, Duration::from_secs(30))
}

#[test]
fn start_constructs_hook_and_lease_backend_adapter() {
  let calls = counter();
  let closed = counter();
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
  assert_eq!(calls.with_read(|calls| *calls), 1);
  assert_eq!(closed.with_read(|closed| *closed), 0);
}

#[test]
fn stop_closes_active_backend_and_rejects_decisions() {
  let calls = counter();
  let closed = counter();
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
  assert_eq!(calls.with_read(|calls| *calls), 0);
  assert_eq!(closed.with_read(|closed| *closed), 1);
}

#[test]
fn drop_closes_active_backend() {
  let calls = counter();
  let closed = counter();
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

  assert_eq!(calls.with_read(|calls| *calls), 0);
  assert_eq!(closed.with_read(|closed| *closed), 1);
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

#[test]
fn downing_provider_decide_context_routes_trait_path_to_lease_backend() {
  let calls = counter();
  let closed = counter();
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
  let mut downing_provider: Box<dyn DowningProvider> = Box::new(provider);

  let decision = downing_provider.decide_context(&majority_context());

  assert_eq!(decision, Ok(DowningDecision::Keep));
  assert_eq!(calls.with_read(|calls| *calls), 1);
}

#[test]
fn downing_provider_trait_path_starts_factory_provider_lazily() {
  let calls = counter();
  let closed = counter();
  let calls_for_factory = calls.clone();
  let closed_for_factory = closed.clone();
  let provider = StdSplitBrainResolverProvider::new(lease_majority_settings()).with_lease_backend_factory(move || {
    Box::new(RecordingLeaseBackend::new(
      LeaseAcquisitionOutcome::Acquired,
      calls_for_factory.clone(),
      closed_for_factory.clone(),
    ))
  });
  let mut downing_provider: Box<dyn DowningProvider> = Box::new(provider);

  let decision = downing_provider.decide_context(&majority_context());

  assert_eq!(decision, Ok(DowningDecision::Keep));
  assert_eq!(calls.with_read(|calls| *calls), 1);
  assert_eq!(closed.with_read(|closed| *closed), 0);
}

#[test]
fn downing_provider_decide_context_preserves_unstable_duration() {
  let mut provider = StdSplitBrainResolverProvider::new(SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));
  provider.start().expect("provider starts");
  let mut downing_provider: Box<dyn DowningProvider> = Box::new(provider);
  let context = majority_context_at(
    TimerInstant::from_ticks(21_000, Duration::from_millis(1)),
    TimerInstant::zero(Duration::from_millis(1)),
  );

  let decision = downing_provider.decide_context(&context);

  assert_eq!(decision, Ok(DowningDecision::Keep));
}

#[test]
fn downing_provider_decide_context_preserves_local_reachability_observer() {
  let mut provider = StdSplitBrainResolverProvider::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));
  provider.start().expect("provider starts");
  let mut downing_provider: Box<dyn DowningProvider> = Box::new(provider);

  let decision = downing_provider.decide_context(&multi_observer_context_with_local_observer());

  assert_eq!(decision, Ok(DowningDecision::Keep));
}
