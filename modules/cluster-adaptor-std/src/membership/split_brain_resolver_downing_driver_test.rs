use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  cluster_provider::ClusterProvider,
  downing_provider::{SplitBrainResolverConfig, SplitBrainResolverStrategy},
  extension::{ClusterProviderError, ClusterProviderShared},
  membership::{MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix},
};
use fraktor_utils_core_rs::time::TimerInstant;

use super::SplitBrainResolverDowningDriver;
use crate::cluster_provider::StdSplitBrainResolverProvider;

#[test]
fn poll_downing_authorities_resets_stable_after_when_unreachable_set_changes() {
  let mut driver = SplitBrainResolverDowningDriver::new(
    StdSplitBrainResolverProvider::new(SplitBrainResolverConfig::new(
      Duration::from_secs(3),
      SplitBrainResolverStrategy::KeepMajority,
      Duration::from_secs(30),
    )),
    "node-a:2552".to_string(),
    ClusterProviderShared::new(Box::new(NoopClusterProvider)),
  );

  let c_unreachable = snapshot(&["node-c:2552"]);
  assert!(driver.poll_downing_authorities(&c_unreachable, now(1)).is_empty());
  assert_eq!(driver.poll_downing_authorities(&c_unreachable, now(4)), vec!["node-c:2552".to_string()]);

  let b_and_c_unreachable = snapshot(&["node-b:2552", "node-c:2552"]);
  assert!(driver.poll_downing_authorities(&b_and_c_unreachable, now(5)).is_empty());
  assert_eq!(driver.poll_downing_authorities(&b_and_c_unreachable, now(8)), vec!["node-a:2552".to_string()]);
}

#[test]
fn poll_downing_authorities_resets_stable_after_when_active_membership_changes() {
  let mut driver = SplitBrainResolverDowningDriver::new(
    StdSplitBrainResolverProvider::new(SplitBrainResolverConfig::new(
      Duration::from_secs(3),
      SplitBrainResolverStrategy::KeepMajority,
      Duration::from_secs(30),
    )),
    "node-a:2552".to_string(),
    ClusterProviderShared::new(Box::new(NoopClusterProvider)),
  );

  let c_unreachable = snapshot(&["node-c:2552"]);
  assert!(driver.poll_downing_authorities(&c_unreachable, now(1)).is_empty());
  assert_eq!(driver.poll_downing_authorities(&c_unreachable, now(4)), vec!["node-c:2552".to_string()]);

  let c_unreachable_with_d =
    snapshot_with_authorities(&["node-a:2552", "node-b:2552", "node-c:2552", "node-d:2552"], &["node-c:2552"]);
  assert!(driver.poll_downing_authorities(&c_unreachable_with_d, now(5)).is_empty());
  assert_eq!(driver.poll_downing_authorities(&c_unreachable_with_d, now(8)), vec!["node-c:2552".to_string()]);
}

struct NoopClusterProvider;

impl ClusterProvider for NoopClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn down(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

fn snapshot(unreachable_authorities: &[&str]) -> MembershipSnapshot {
  snapshot_with_authorities(&["node-a:2552", "node-b:2552", "node-c:2552"], unreachable_authorities)
}

fn snapshot_with_authorities(authorities: &[&str], unreachable_authorities: &[&str]) -> MembershipSnapshot {
  let records = authorities.iter().map(|authority| record_from_authority(authority)).collect::<Vec<_>>();
  let local = records[0].unique_address.clone();
  let mut reachability = ReachabilityMatrix::new();
  for authority in unreachable_authorities {
    let subject =
      records.iter().find(|record| record.authority == *authority).expect("subject record").unique_address.clone();
    reachability.unreachable(local.clone(), subject);
  }
  MembershipSnapshot::new_with_reachability(MembershipVersion::new(1), records, reachability.snapshot())
}

fn record_from_authority(authority: &str) -> NodeRecord {
  let node_id = authority.split(':').next().unwrap_or(authority);
  record(node_id, authority)
}

fn record(node_id: &str, authority: &str) -> NodeRecord {
  NodeRecord::new(
    node_id.to_string(),
    authority.to_string(),
    NodeStatus::Up,
    MembershipVersion::new(1),
    "1.0.0".to_string(),
    Vec::new(),
  )
}

const fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}
