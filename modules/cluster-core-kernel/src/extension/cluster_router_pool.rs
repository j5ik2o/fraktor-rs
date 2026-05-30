//! Minimal pool router for cluster routee authorities.

#[cfg(test)]
#[path = "cluster_router_pool_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use crate::{ClusterRouterPoolConfig, membership::NodeRecord};

/// Round-robin pool router for cluster routees.
pub struct ClusterRouterPool {
  config:     ClusterRouterPoolConfig,
  routees:    Vec<String>,
  next_index: usize,
}

impl ClusterRouterPool {
  /// Creates a pool router with config and initial routees.
  #[must_use]
  pub const fn new(config: ClusterRouterPoolConfig, routees: Vec<String>) -> Self {
    Self { config, routees, next_index: 0 }
  }

  /// Returns the router config.
  #[must_use]
  pub const fn config(&self) -> &ClusterRouterPoolConfig {
    &self.config
  }

  /// Returns the current routees.
  #[must_use]
  pub fn routees(&self) -> &[String] {
    &self.routees
  }

  /// Replaces current routees.
  pub fn replace_routees(&mut self, routees: Vec<String>) {
    self.routees = routees;
    self.next_index = 0;
  }

  /// Rebuilds routees from active membership records.
  pub fn replace_routees_from_members(&mut self, members: &[NodeRecord], local_authority: Option<&str>) {
    let mut routees = Vec::new();

    for member in members.iter().filter(|member| self.accepts_member(member, local_authority)) {
      let remaining = self.config.total_instances().saturating_sub(routees.len());
      if remaining == 0 {
        break;
      }

      let instances = self.instances_for_member(remaining);
      for _ in 0..instances {
        routees.push(member.authority.clone());
      }
    }

    self.replace_routees(routees);
  }

  /// Selects the next routee authority using round-robin.
  ///
  /// The effective pool is capped at [`ClusterRouterPoolConfig::total_instances`].
  #[must_use]
  pub fn next_routee(&mut self) -> Option<&str> {
    if self.routees.is_empty() {
      return None;
    }
    let effective_count = self.routees.len().min(self.config.total_instances());
    let index = self.next_index % effective_count;
    self.next_index = (self.next_index + 1) % effective_count;
    Some(self.routees[index].as_str())
  }

  fn accepts_member(&self, member: &NodeRecord, local_authority: Option<&str>) -> bool {
    if !member.status.is_active() {
      return false;
    }
    if !self.config.allow_local_routees()
      && local_authority.is_some_and(|authority| authority == member.authority.as_str())
    {
      return false;
    }
    self.config.use_roles().is_empty() || self.config.use_roles().iter().any(|role| member.roles.contains(role))
  }

  fn instances_for_member(&self, remaining: usize) -> usize {
    self.config.max_instances_per_node().unwrap_or(1).min(remaining)
  }
}
