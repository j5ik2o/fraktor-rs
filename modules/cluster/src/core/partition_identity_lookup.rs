//! Partition-based identity lookup using distributed hashing.

use alloc::{string::String, vec::Vec};

use crate::core::{
  activated_kind::ActivatedKind, grain_key::GrainKey, identity_lookup::IdentityLookup,
  identity_setup_error::IdentitySetupError, partition_identity_lookup_config::PartitionIdentityLookupConfig,
  pid_cache_event::PidCacheEvent, virtual_actor_event::VirtualActorEvent, virtual_actor_registry::VirtualActorRegistry,
};

#[cfg(test)]
mod tests;

/// Distributed hash-based identity lookup implementation.
///
/// This component resolves grain keys to PIDs using rendezvous hashing
/// to select owner nodes. All methods that modify state use `&mut self`,
/// and callers should wrap the instance in `ToolboxMutex<Box<dyn IdentityLookup>>`
/// for thread-safe access.
pub struct PartitionIdentityLookup {
  /// Virtual actor registry for activation management (includes PidCache).
  registry:     VirtualActorRegistry,
  /// Current list of active authorities.
  authorities:  Vec<String>,
  /// Registered activated kinds for member mode.
  member_kinds: Vec<ActivatedKind>,
  /// Registered activated kinds for client mode.
  client_kinds: Vec<ActivatedKind>,
  /// Configuration parameters.
  config:       PartitionIdentityLookupConfig,
}

impl PartitionIdentityLookup {
  /// Creates a new partition identity lookup with the given configuration.
  #[must_use]
  pub const fn new(config: PartitionIdentityLookupConfig) -> Self {
    let cache_capacity = config.cache_capacity();
    let pid_ttl_secs = config.pid_ttl_secs();
    Self {
      registry: VirtualActorRegistry::new(cache_capacity, pid_ttl_secs),
      authorities: Vec::new(),
      member_kinds: Vec::new(),
      client_kinds: Vec::new(),
      config,
    }
  }

  /// Creates a new partition identity lookup with default configuration.
  #[must_use]
  pub fn with_defaults() -> Self {
    Self::new(PartitionIdentityLookupConfig::default())
  }

  /// Returns the current authority list.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn authorities(&self) -> &[String] {
    &self.authorities
  }

  /// Returns the configuration.
  #[must_use]
  pub const fn config(&self) -> &PartitionIdentityLookupConfig {
    &self.config
  }

  /// Returns the registered member kinds.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn member_kinds(&self) -> &[ActivatedKind] {
    &self.member_kinds
  }

  /// Returns the registered client kinds.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn client_kinds(&self) -> &[ActivatedKind] {
    &self.client_kinds
  }
}

impl IdentityLookup for PartitionIdentityLookup {
  fn setup_member(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.member_kinds = kinds.to_vec();
    Ok(())
  }

  fn setup_client(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.client_kinds = kinds.to_vec();
    Ok(())
  }

  fn get(&mut self, key: &GrainKey, now: u64) -> Option<String> {
    // Step 1: キャッシュを確認（高速パス、イベント生成なし）
    if let Some(pid) = self.registry.cached_pid(key, now) {
      return Some(pid);
    }

    // Step 2: VirtualActorRegistry 経由でアクティベーションを確保
    // snapshot_required は false、snapshot は None で基本的なルックアップを行う
    match self.registry.ensure_activation(key, &self.authorities, now, false, None) {
      | Ok(pid) => Some(pid),
      | Err(_) => None,
    }
  }

  fn remove_pid(&mut self, key: &GrainKey) {
    self.registry.remove_activation(key);
  }

  fn update_topology(&mut self, authorities: Vec<String>) {
    // 新しい authority リストに存在しないものを無効化
    self.registry.invalidate_absent_authorities(&authorities);
    // 内部の authority リストを更新
    self.authorities = authorities;
  }

  fn on_member_left(&mut self, authority: &str) {
    // 指定された authority のエントリをすべて無効化
    self.registry.invalidate_authority(authority);
  }

  fn passivate_idle(&mut self, now: u64, idle_ttl: u64) {
    self.registry.passivate_idle(now, idle_ttl);
  }

  fn drain_events(&mut self) -> Vec<VirtualActorEvent> {
    self.registry.drain_events()
  }

  fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    self.registry.drain_cache_events()
  }
}
