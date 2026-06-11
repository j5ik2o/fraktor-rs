//! Distributed pub-sub mediator configuration.

#[cfg(test)]
#[path = "distributed_pub_sub_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use super::{PubSubError, PubSubNoSubscriberBehavior, PubSubRoutingMode};
use crate::membership::{CurrentClusterState, NodeRecord};

/// Configuration for distributed pub-sub mediator behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedPubSubConfig {
  role:                   Option<String>,
  routing_mode:           PubSubRoutingMode,
  gossip_interval:        Duration,
  removed_entry_ttl:      Duration,
  max_delta_elements:     usize,
  no_subscriber_behavior: PubSubNoSubscriberBehavior,
}

impl DistributedPubSubConfig {
  /// Creates a configuration after validating bounded-delta parameters.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidConfig`] when `max_delta_elements` is zero.
  pub fn try_new(
    role: Option<String>,
    routing_mode: PubSubRoutingMode,
    gossip_interval: Duration,
    removed_entry_ttl: Duration,
    max_delta_elements: usize,
    no_subscriber_behavior: PubSubNoSubscriberBehavior,
  ) -> Result<Self, PubSubError> {
    if max_delta_elements == 0 {
      return Err(PubSubError::InvalidConfig {
        reason: String::from("max_delta_elements must be greater than zero"),
      });
    }

    Ok(Self { role, routing_mode, gossip_interval, removed_entry_ttl, max_delta_elements, no_subscriber_behavior })
  }

  /// Returns the optional mediator role filter.
  #[must_use]
  pub const fn role(&self) -> Option<&String> {
    self.role.as_ref()
  }

  /// Returns the path `Send` routing mode.
  #[must_use]
  pub const fn routing_mode(&self) -> PubSubRoutingMode {
    self.routing_mode
  }

  /// Returns the registry gossip interval.
  #[must_use]
  pub const fn gossip_interval(&self) -> Duration {
    self.gossip_interval
  }

  /// Returns the removed registry entry retention TTL.
  #[must_use]
  pub const fn removed_entry_ttl(&self) -> Duration {
    self.removed_entry_ttl
  }

  /// Returns the maximum number of registry entries in one delta.
  #[must_use]
  pub const fn max_delta_elements(&self) -> usize {
    self.max_delta_elements
  }

  /// Returns how delivery behaves when no target exists.
  #[must_use]
  pub const fn no_subscriber_behavior(&self) -> PubSubNoSubscriberBehavior {
    self.no_subscriber_behavior
  }

  /// Filters active mediator candidates from the current membership view.
  #[must_use]
  pub fn mediator_candidates(&self, state: &CurrentClusterState) -> Vec<NodeRecord> {
    state
      .members
      .iter()
      .filter(|record| record.status.is_active())
      .filter(|record| self.role.as_ref().is_none_or(|role| record.roles.iter().any(|candidate| candidate == role)))
      .cloned()
      .collect()
  }
}

impl Default for DistributedPubSubConfig {
  fn default() -> Self {
    Self {
      role:                   None,
      routing_mode:           PubSubRoutingMode::default(),
      gossip_interval:        Duration::from_secs(1),
      removed_entry_ttl:      Duration::from_secs(120),
      max_delta_elements:     3000,
      no_subscriber_behavior: PubSubNoSubscriberBehavior::default(),
    }
  }
}
