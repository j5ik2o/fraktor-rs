use alloc::{string::String, vec};
use core::{slice, time::Duration};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  downing_provider::{
    DowningDecision, DowningDecisionContext, LeaseAcquisitionOutcome, LeaseMajorityPort, SplitBrainResolver,
    SplitBrainResolverSettings, SplitBrainResolverStrategy,
  },
  membership::{DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix},
};

struct StaticLeasePort {
  outcome: LeaseAcquisitionOutcome,
  calls:   usize,
}

impl LeaseMajorityPort for StaticLeasePort {
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

fn context(snapshot: MembershipSnapshot) -> DowningDecisionContext {
  DowningDecisionContext::from_membership_snapshot(snapshot, TimerInstant::zero(Duration::from_millis(1)))
}

fn context_with_observer(snapshot: MembershipSnapshot, observer: UniqueAddress) -> DowningDecisionContext {
  context(snapshot).with_reachability_observer(observer)
}

#[test]
fn keep_majority_keeps_reachable_majority_partition() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone(), node_c.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::KeepMajority);
  assert_eq!(decision.retained_partition(), &[node_a.unique_address.clone(), node_b.unique_address.clone()]);
  assert_eq!(decision.downing_targets(), slice::from_ref(&node_c.unique_address));
}

#[test]
fn keep_majority_defers_when_multi_observer_partition_lacks_local_observer() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  reachability.unreachable(node_b.unique_address.clone(), node_a.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(
    decision.trace().reason(),
    "local reachability observer is required for multi-observer membership evaluation"
  );
}

#[test]
fn keep_majority_uses_local_observer_partition_for_symmetric_split() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  reachability.unreachable(node_b.unique_address.clone(), node_a.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context_with_observer(snapshot, node_a.unique_address));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().tie_break_rule(), Some("reachable and non-reachable partitions have equal size"));
}

#[test]
fn static_quorum_uses_configured_fixed_quorum() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::WeaklyUp, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let node_d = record("node-d", 4, NodeStatus::Up, 4);
  let node_e = record("node-e", 5, NodeStatus::Up, 5);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  reachability.unreachable(node_a.unique_address.clone(), node_d.unique_address.clone());
  reachability.unreachable(node_a.unique_address.clone(), node_e.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone(), node_c.clone(), node_d.clone(), node_e.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::StaticQuorum, Duration::from_secs(30))
      .with_static_quorum_size(2),
  );

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::StaticQuorum);
  assert_eq!(decision.retained_partition(), &[node_a.unique_address.clone(), node_b.unique_address.clone()]);
  assert_eq!(decision.downing_targets(), &[
    node_c.unique_address.clone(),
    node_d.unique_address.clone(),
    node_e.unique_address.clone()
  ]);
}

#[test]
fn static_quorum_without_configured_size_defers() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.reachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::StaticQuorum,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::StaticQuorum);
  assert_eq!(decision.trace().reason(), "static quorum size is not configured");
}

#[test]
fn static_quorum_with_zero_size_defers() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.reachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(
    SplitBrainResolverSettings::new(Duration::ZERO, SplitBrainResolverStrategy::StaticQuorum, Duration::from_secs(30))
      .with_static_quorum_size(0),
  );

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::StaticQuorum);
  assert_eq!(decision.trace().reason(), "static quorum size must be greater than zero");
}

#[test]
fn majority_tie_defers_with_tie_break_reason() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepMajority,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().tie_break_rule(), Some("reachable and non-reachable partitions have equal size"));
}

#[test]
fn stable_after_nonzero_defers_before_strategy_evaluation() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.reachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::from_secs(20),
    SplitBrainResolverStrategy::KeepOldest,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::KeepOldest);
  assert_eq!(decision.trace().stable_after_required(), Some(Duration::from_secs(20)));
}

#[test]
fn keep_oldest_retains_partition_containing_oldest_member() {
  let node_a = record("node-a", 1, NodeStatus::Up, 10);
  let node_b = record("node-b", 2, NodeStatus::Up, 1);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::KeepOldest,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::KeepOldest);
  assert_eq!(decision.retained_partition(), slice::from_ref(&node_b.unique_address));
  assert_eq!(decision.downing_targets(), slice::from_ref(&node_a.unique_address));
}

#[test]
fn down_all_defers_until_timeout_is_elapsed() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::DownAll,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::DownAll);
  assert_eq!(decision.trace().down_all_timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn down_all_returns_all_down_when_timeout_is_elapsed() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::DownAll,
    Duration::ZERO,
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Down);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::DownAll);
  assert_eq!(decision.downing_targets(), &[node_a.unique_address.clone(), node_b.unique_address.clone()]);
  assert!(decision.is_all_down());
}

#[test]
fn lease_majority_keeps_majority_partition_only_when_lease_is_acquired() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let node_c = record("node-c", 3, NodeStatus::Up, 3);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_c.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone(), node_c.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::LeaseMajority,
    Duration::from_secs(30),
  ));
  let mut lease = StaticLeasePort { outcome: LeaseAcquisitionOutcome::Acquired, calls: 0 };

  let decision = resolver.decide_with_lease(&context(snapshot), &mut lease);

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().strategy(), SplitBrainResolverStrategy::LeaseMajority);
  assert_eq!(decision.trace().lease_outcome(), Some(LeaseAcquisitionOutcome::Acquired));
  assert_eq!(decision.retained_partition(), &[node_a.unique_address.clone(), node_b.unique_address.clone()]);
  assert_eq!(decision.downing_targets(), slice::from_ref(&node_c.unique_address));
  assert_eq!(lease.calls, 1);
}

#[test]
fn lease_majority_defers_when_lease_is_not_acquired() {
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
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::LeaseMajority,
    Duration::from_secs(30),
  ));
  let mut lease = StaticLeasePort { outcome: LeaseAcquisitionOutcome::Denied, calls: 0 };

  let decision = resolver.decide_with_lease(&context(snapshot), &mut lease);

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().lease_outcome(), Some(LeaseAcquisitionOutcome::Denied));
  assert!(decision.retained_partition().is_empty());
  assert!(decision.downing_targets().is_empty());
  assert_eq!(lease.calls, 1);
}

#[test]
fn lease_majority_consults_lease_when_partitions_are_tied() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.unreachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a.clone(), node_b.clone()],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::LeaseMajority,
    Duration::from_secs(30),
  ));
  let mut lease = StaticLeasePort { outcome: LeaseAcquisitionOutcome::Acquired, calls: 0 };

  let decision = resolver.decide_with_lease(&context(snapshot), &mut lease);

  assert_eq!(decision.simple_decision(), DowningDecision::Keep);
  assert_eq!(decision.trace().lease_outcome(), Some(LeaseAcquisitionOutcome::Acquired));
  assert_eq!(decision.retained_partition(), slice::from_ref(&node_a.unique_address));
  assert_eq!(decision.downing_targets(), slice::from_ref(&node_b.unique_address));
  assert_eq!(lease.calls, 1);
}

#[test]
fn lease_majority_without_backend_defers_with_backend_missing_outcome() {
  let node_a = record("node-a", 1, NodeStatus::Up, 1);
  let node_b = record("node-b", 2, NodeStatus::Up, 2);
  let mut reachability = ReachabilityMatrix::new();
  reachability.reachable(node_a.unique_address.clone(), node_b.unique_address.clone());
  let snapshot = MembershipSnapshot::new_with_reachability(
    MembershipVersion::new(10),
    vec![node_a, node_b],
    reachability.snapshot(),
  );
  let resolver = SplitBrainResolver::new(SplitBrainResolverSettings::new(
    Duration::ZERO,
    SplitBrainResolverStrategy::LeaseMajority,
    Duration::from_secs(30),
  ));

  let decision = resolver.decide(&context(snapshot));

  assert_eq!(decision.simple_decision(), DowningDecision::Defer);
  assert_eq!(decision.trace().lease_outcome(), Some(LeaseAcquisitionOutcome::BackendMissing));
}
