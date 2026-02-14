//! Partition-based identity lookup using distributed hashing.

use alloc::{format, string::String, vec::Vec};

use super::{
  identity_lookup::IdentityLookup, identity_setup_error::IdentitySetupError, lookup_error::LookupError,
  partition_identity_lookup_config::PartitionIdentityLookupConfig, pid_cache_event::PidCacheEvent,
};
use crate::core::{
  grain::GrainKey,
  placement::{
    ActivatedKind, PlacementCommandResult, PlacementCoordinatorCore, PlacementCoordinatorError,
    PlacementCoordinatorOutcome, PlacementEvent, PlacementResolution,
  },
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
  /// Placement coordinator core.
  coordinator:  PlacementCoordinatorCore,
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
    Self {
      coordinator: PlacementCoordinatorCore::new(config.cache_capacity(), config.pid_ttl_secs()),
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
    self.coordinator.authorities()
  }

  /// Returns the configuration.
  #[must_use]
  pub const fn config(&self) -> &PartitionIdentityLookupConfig {
    &self.config
  }

  /// Sets the local authority identifier.
  pub fn set_local_authority(&mut self, authority: impl Into<String>) {
    self.coordinator.set_local_authority(authority);
  }

  /// Enables or disables distributed activation commands.
  pub const fn set_distributed_activation(&mut self, enabled: bool) {
    self.coordinator.set_distributed_activation(enabled);
  }

  /// Handles a command result and records emitted events.
  ///
  /// # Errors
  ///
  /// Returns an error if the coordinator rejects the result.
  pub fn handle_command_result(
    &mut self,
    result: PlacementCommandResult,
  ) -> Result<PlacementCoordinatorOutcome, PlacementCoordinatorError> {
    self.coordinator.handle_command_result(result)
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
    self.coordinator.start_member().map_err(|error| IdentitySetupError::Provider(format!("{error:?}")))?;
    Ok(())
  }

  fn setup_client(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    self.client_kinds = kinds.to_vec();
    self.coordinator.start_client().map_err(|error| IdentitySetupError::Provider(format!("{error:?}")))?;
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let outcome = self.coordinator.resolve(key, now)?;
    if let Some(resolution) = outcome.resolution {
      return Ok(resolution);
    }
    Err(LookupError::Pending)
  }

  fn remove_pid(&mut self, key: &GrainKey) {
    self.coordinator.remove_pid(key);
  }

  fn update_topology(&mut self, authorities: Vec<String>) {
    self.coordinator.update_topology(authorities);
  }

  fn on_member_left(&mut self, authority: &str) {
    self.coordinator.invalidate_authority(authority);
  }

  fn passivate_idle(&mut self, now: u64, idle_ttl: u64) {
    self.coordinator.passivate_idle(now, idle_ttl);
  }

  fn drain_events(&mut self) -> Vec<PlacementEvent> {
    self.coordinator.drain_events()
  }

  fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    self.coordinator.drain_cache_events()
  }
}
