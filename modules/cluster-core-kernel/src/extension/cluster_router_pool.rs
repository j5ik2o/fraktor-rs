//! Minimal pool router for cluster routee authorities.

#[cfg(test)]
#[path = "cluster_router_pool_test.rs"]
mod tests;

use alloc::{string::String, vec, vec::Vec};

use crate::{
  ClusterRouterPoolConfig,
  membership::{NodeRecord, NodeStatus},
};

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

  /// Creates a pool router by allocating routees across the given candidate node
  /// authorities.
  ///
  /// Candidates are expected to be distinct node authorities, pre-filtered by
  /// [`ClusterRouterPoolConfig::satisfies_roles`] and node availability. Routees
  /// are distributed least-loaded first, capped at
  /// [`ClusterRouterPoolConfig::total_instances`] in total and at
  /// [`ClusterRouterPoolConfig::max_instances_per_node`] per authority. Ties for
  /// the least-loaded authority are broken in favor of the earliest entry in
  /// `candidates`, so the allocation is deterministic for a given candidate
  /// order.
  #[must_use]
  pub fn from_candidates(config: ClusterRouterPoolConfig, candidates: &[String]) -> Self {
    let routees = allocate_routees(&config, candidates);
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

  /// Recomputes the routee set from the current cluster membership snapshot.
  ///
  /// A member contributes its authority as a routee target only when it is `Up`,
  /// carries every required role
  /// ([`ClusterRouterPoolConfig::satisfies_roles`]), and is not excluded by the
  /// local-routee policy ([`ClusterRouterPoolConfig::allow_local_routees`]). The
  /// surviving distinct authorities are then placed honoring the total and
  /// per-node caps, exactly as in [`ClusterRouterPool::from_candidates`].
  ///
  /// This is the core routing policy. The core cluster runtime drives it on
  /// membership changes, obtaining `ClusterEvent` snapshots through a core port
  /// whose std adapter is injected via DI (core drives the adapter, never the
  /// reverse).
  pub fn update_from_members(&mut self, members: &[NodeRecord], self_authority: &str) {
    let mut candidates: Vec<String> = Vec::new();
    for member in members {
      if member.status != NodeStatus::Up {
        continue;
      }
      if !self.config.satisfies_roles(&member.roles) {
        continue;
      }
      if !self.config.allow_local_routees() && member.authority == self_authority {
        continue;
      }
      if !candidates.iter().any(|authority| authority == &member.authority) {
        candidates.push(member.authority.clone());
      }
    }
    self.routees = allocate_routees(&self.config, &candidates);
    self.next_index = 0;
  }

  /// Selects the next routee authority using round-robin.
  ///
  /// The effective pool is capped at [`ClusterRouterPoolConfig::total_instances`].
  // NOTE: CQS 違反の根拠: round-robin セレクタはカーソルを前進させつつ選択結果を返す
  // 必要があり、読み取りと更新を分離できない。これは `cqs-principle.md` が許容例外
  // として明示する `Iterator::next` / `Vec::pop` 相当のケースであり、本変更の計画
  // レビューで人間の許可を取得済み。
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

/// Distributes routees across candidate authorities honoring the total and
/// per-node caps, using least-loaded round-robin placement.
fn allocate_routees(config: &ClusterRouterPoolConfig, candidates: &[String]) -> Vec<String> {
  let total = config.total_instances();
  let max_per_node = config.max_instances_per_node();
  if candidates.is_empty() {
    return Vec::new();
  }
  let mut counts = vec![0usize; candidates.len()];
  let mut routees: Vec<String> = Vec::new();
  while routees.len() < total {
    let mut best: Option<usize> = None;
    for (index, &count) in counts.iter().enumerate() {
      if count >= max_per_node {
        continue;
      }
      match best {
        | Some(best_index) if counts[best_index] <= count => {},
        | _ => best = Some(index),
      }
    }
    match best {
      | Some(index) => {
        routees.push(candidates[index].clone());
        counts[index] += 1;
      },
      | None => break,
    }
  }
  routees
}
