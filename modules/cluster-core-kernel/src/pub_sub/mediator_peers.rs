//! Pub-sub mediator peers derived from membership state.

#[cfg(test)]
#[path = "mediator_peers_test.rs"]
mod tests;

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::DistributedPubSubConfig;
use crate::membership::CurrentClusterState;

/// Active mediator owner identities selected from membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediatorPeers {
  active_owners: Vec<UniqueAddress>,
}

impl MediatorPeers {
  /// Creates mediator peers from active membership records and the role filter.
  #[must_use]
  pub fn from_state(settings: &DistributedPubSubConfig, state: &CurrentClusterState) -> Self {
    Self {
      active_owners: settings.mediator_candidates(state).into_iter().map(|record| record.unique_address).collect(),
    }
  }

  /// Creates mediator peers from already selected owner identities.
  #[must_use]
  pub const fn new(active_owners: Vec<UniqueAddress>) -> Self {
    Self { active_owners }
  }

  /// Returns active owner identities.
  #[must_use]
  pub fn active_owners(&self) -> &[UniqueAddress] {
    &self.active_owners
  }

  /// Returns true when an owner may be used for delivery or delta application.
  #[must_use]
  pub fn contains(&self, owner: &UniqueAddress) -> bool {
    self.active_owners.iter().any(|candidate| candidate == owner)
  }
}
