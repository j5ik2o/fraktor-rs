//! Cluster Singleton proxy location tracking state.

#[cfg(test)]
#[path = "cluster_singleton_proxy_test.rs"]
mod tests;

use alloc::{collections::VecDeque, string::String, vec, vec::Vec};

use super::ClusterSingletonProxyConfig;
use crate::membership::{DataCenter, NodeRecord, NodeStatus, oldest_member};

/// Effect requested by the proxy state machine for the runtime driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterSingletonProxyEffect<M> {
  /// Forward a message to the identified singleton location.
  Forward {
    /// Target authority hosting the singleton.
    location: String,
    /// Message to deliver.
    message:  M,
  },
  /// Buffer a message until the singleton location is identified.
  Buffer {
    /// Message to buffer.
    message: M,
  },
  /// Drop a message because buffering is disabled or full.
  Drop {
    /// Message that was dropped.
    message: M,
  },
  /// Trigger singleton identification against current membership.
  Identify,
}

/// Outcome produced by applying proxy input to the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSingletonProxyOutcome<M> {
  /// Effects for the runtime driver to execute.
  pub effects: Vec<ClusterSingletonProxyEffect<M>>,
}

impl<M> ClusterSingletonProxyOutcome<M> {
  fn with_effect(effect: ClusterSingletonProxyEffect<M>) -> Self {
    Self { effects: vec![effect] }
  }

  fn empty() -> Self {
    Self { effects: Vec::new() }
  }
}

/// Pure state machine tracking singleton location and proxy buffering.
#[derive(Debug, Clone)]
pub struct ClusterSingletonProxy<M> {
  config:              ClusterSingletonProxyConfig,
  identified_location: Option<String>,
  buffer:              VecDeque<M>,
}

impl<M> ClusterSingletonProxy<M> {
  /// Creates a proxy with the provided configuration.
  #[must_use]
  pub fn new(config: ClusterSingletonProxyConfig) -> Self {
    Self { config, identified_location: None, buffer: VecDeque::new() }
  }

  /// Returns the configured proxy settings.
  #[must_use]
  pub const fn config(&self) -> &ClusterSingletonProxyConfig {
    &self.config
  }

  /// Returns the currently identified singleton location, if any.
  #[must_use]
  pub fn identified_location(&self) -> Option<&str> {
    self.identified_location.as_deref()
  }

  /// Returns the number of buffered messages.
  #[must_use]
  pub fn buffered_count(&self) -> usize {
    self.buffer.len()
  }

  /// Updates singleton location from the current membership snapshot.
  #[must_use]
  pub fn identify(
    &mut self,
    members: &[NodeRecord],
    local_data_center: &DataCenter,
  ) -> ClusterSingletonProxyOutcome<M> {
    let eligible =
      eligible_members(members, self.config.role(), self.config.data_center().unwrap_or(local_data_center));
    self.identified_location = oldest_member(&eligible).map(|record| record.authority.clone());

    let mut effects = Vec::new();
    if self.identified_location.is_some() {
      while let Some(message) = self.buffer.pop_front() {
        effects.push(ClusterSingletonProxyEffect::Forward {
          location: self.identified_location.clone().expect("location set after identify"),
          message,
        });
      }
    } else {
      effects.push(ClusterSingletonProxyEffect::Identify);
    }
    ClusterSingletonProxyOutcome { effects }
  }

  /// Handles an outbound proxy message.
  #[must_use]
  pub fn handle_message(&mut self, message: M) -> ClusterSingletonProxyOutcome<M> {
    if let Some(location) = self.identified_location.clone() {
      return ClusterSingletonProxyOutcome::with_effect(ClusterSingletonProxyEffect::Forward { location, message });
    }

    let buffer_size = self.config.buffer_size();
    if buffer_size == 0 {
      return ClusterSingletonProxyOutcome::with_effect(ClusterSingletonProxyEffect::Drop { message });
    }

    if self.buffer.len() >= usize::try_from(buffer_size).unwrap_or(usize::MAX) {
      let _dropped = self.buffer.pop_front();
    }
    self.buffer.push_back(message);
    ClusterSingletonProxyOutcome::empty()
  }
}

fn eligible_members(members: &[NodeRecord], role: Option<&str>, data_center: &DataCenter) -> Vec<NodeRecord> {
  members
    .iter()
    .filter(|record| record.status == NodeStatus::Up)
    .filter(|record| &record.data_center == data_center)
    .filter(|record| role.is_none_or(|role| record.roles.iter().any(|candidate| candidate == role)))
    .cloned()
    .collect()
}
