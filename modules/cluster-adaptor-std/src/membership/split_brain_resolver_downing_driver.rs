//! std runtime bridge that executes Split Brain Resolver downing targets.

#[cfg(test)]
#[path = "split_brain_resolver_downing_driver_test.rs"]
mod tests;

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use fraktor_cluster_core_kernel_rs::{
  downing_provider::DowningDecisionContext,
  extension::{ClusterProviderError, ClusterProviderShared},
  membership::{MembershipSnapshot, MembershipVersion, NodeRecord, ReachabilityStatus},
};
use fraktor_remote_core_rs::address::UniqueAddress;
use fraktor_utils_core_rs::{sync::SharedAccess, time::TimerInstant};

use crate::cluster_provider::StdSplitBrainResolverProvider;

pub(super) struct SplitBrainResolverDowningDriver {
  provider:             StdSplitBrainResolverProvider,
  local_authority:      String,
  cluster_provider:     ClusterProviderShared,
  unstable_observation: Option<UnstableObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UnstableObservation {
  active_members:      BTreeSet<ActiveMemberObservation>,
  unreachable_members: BTreeSet<UniqueAddress>,
  since:               TimerInstant,
}

type ActiveMemberObservation = (UniqueAddress, MembershipVersion);

impl SplitBrainResolverDowningDriver {
  pub(super) fn new(
    provider: StdSplitBrainResolverProvider,
    local_authority: String,
    cluster_provider: ClusterProviderShared,
  ) -> Self {
    Self { provider, local_authority, cluster_provider, unstable_observation: None }
  }

  pub(super) fn poll_downing_authorities(&mut self, snapshot: &MembershipSnapshot, now: TimerInstant) -> Vec<String> {
    let Some(local_record) = self.local_record(snapshot) else {
      self.unstable_observation = None;
      return Vec::new();
    };
    let unreachable_members = local_unreachable_members(snapshot, &local_record.unique_address);
    if unreachable_members.is_empty() {
      self.unstable_observation = None;
      return Vec::new();
    }

    let active_members = active_members(snapshot);
    let unstable_since = self.unstable_since(active_members, unreachable_members, now);
    let context = DowningDecisionContext::from_membership_snapshot(snapshot.clone(), now)
      .with_reachability_observer(local_record.unique_address.clone())
      .with_unstable_since(unstable_since);

    let decision = match self.provider.decide_strategy_context(&context) {
      | Ok(decision) => decision,
      | Err(error) => {
        tracing::warn!(reason = %error.reason(), "split-brain-resolver downing decision failed");
        return Vec::new();
      },
    };
    downing_authorities(snapshot, decision.downing_targets())
  }

  fn local_record(&self, snapshot: &MembershipSnapshot) -> Option<NodeRecord> {
    snapshot
      .entries
      .iter()
      .find(|record| record.authority == self.local_authority && record.status.is_active())
      .cloned()
  }

  fn unstable_since(
    &mut self,
    active_members: BTreeSet<ActiveMemberObservation>,
    unreachable_members: BTreeSet<UniqueAddress>,
    now: TimerInstant,
  ) -> TimerInstant {
    if let Some(observation) = self.unstable_observation.as_ref()
      && observation.active_members == active_members
      && observation.unreachable_members == unreachable_members
    {
      return observation.since;
    }
    self.unstable_observation = Some(UnstableObservation { active_members, unreachable_members, since: now });
    now
  }

  pub(super) fn down_cluster_provider(&self, authority: &str) -> Result<(), ClusterProviderError> {
    self.cluster_provider.with_write(|provider| provider.down(authority)).inspect_err(|error| {
      tracing::warn!(
        target = %authority,
        reason = %error.reason(),
        "split-brain-resolver cluster provider downing target execution failed"
      );
    })
  }

  pub(super) fn is_local_authority(&self, authority: &str) -> bool {
    self.local_authority == authority
  }
}

fn active_members(snapshot: &MembershipSnapshot) -> BTreeSet<ActiveMemberObservation> {
  snapshot
    .entries
    .iter()
    .filter(|record| record.status.is_active())
    .map(|record| (record.unique_address.clone(), record.join_version))
    .collect()
}

fn local_unreachable_members(snapshot: &MembershipSnapshot, observer: &UniqueAddress) -> BTreeSet<UniqueAddress> {
  if !snapshot.reachability.has_observer(observer) {
    return BTreeSet::new();
  }
  snapshot
    .entries
    .iter()
    .filter(|record| record.status.is_active() && &record.unique_address != observer)
    .filter(|record| {
      matches!(
        snapshot.reachability.observed_status(observer, &record.unique_address),
        Some(ReachabilityStatus::Unreachable | ReachabilityStatus::Terminated)
      )
    })
    .map(|record| record.unique_address.clone())
    .collect()
}

fn downing_authorities(snapshot: &MembershipSnapshot, targets: &[UniqueAddress]) -> Vec<String> {
  snapshot
    .entries
    .iter()
    .filter(|record| targets.iter().any(|target| target == &record.unique_address))
    .map(|record| record.authority.clone())
    .collect()
}
