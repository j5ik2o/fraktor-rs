//! Identity lookup abstraction for cluster modes.

use alloc::{string::String, vec::Vec};

use crate::core::{
  activated_kind::ActivatedKind, grain_key::GrainKey, identity_setup_error::IdentitySetupError,
  pid_cache_event::PidCacheEvent, virtual_actor_event::VirtualActorEvent,
};

/// Provides identity resolution setup and lookup operations.
///
/// All methods that modify internal state use `&mut self` to make state changes
/// explicit in the type signature. Callers (such as `ClusterCore`) should wrap
/// the implementation in `ToolboxMutex<Box<dyn IdentityLookup>>` for thread-safe access.
pub trait IdentityLookup: Send + Sync {
  /// Prepares identity lookup for member mode with the provided kinds.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails for member mode.
  fn setup_member(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError>;

  /// Prepares identity lookup for client mode with the provided kinds.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails for client mode.
  fn setup_client(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError>;

  /// Resolves the PID for a grain key.
  ///
  /// Returns `Some(pid)` if the grain is active or can be activated,
  /// `None` if no authority is available or activation failed.
  ///
  /// Note: This method uses `&mut self` because it may update the cache
  /// and create new activations as side effects.
  ///
  /// # Arguments
  ///
  /// * `key` - The grain key to resolve
  /// * `now` - Current Unix timestamp in seconds for TTL calculation
  fn get(&mut self, key: &GrainKey, now: u64) -> Option<String> {
    let _ = (key, now);
    None
  }

  /// Removes a PID from the registry and cache.
  ///
  /// # Arguments
  ///
  /// * `key` - The grain key to remove
  fn remove_pid(&mut self, key: &GrainKey) {
    let _ = key;
  }

  /// Updates the authority list based on topology changes.
  ///
  /// This invalidates activations and cache entries for authorities
  /// that are no longer present.
  ///
  /// # Arguments
  ///
  /// * `authorities` - Current list of active authorities
  fn update_topology(&mut self, authorities: Vec<String>) {
    let _ = authorities;
  }

  /// Handles a member leaving the cluster.
  ///
  /// Invalidates all activations and cache entries for the given authority.
  ///
  /// # Arguments
  ///
  /// * `authority` - The authority address that left
  fn on_member_left(&mut self, authority: &str) {
    let _ = authority;
  }

  /// Passivates idle activations that exceed the given TTL.
  ///
  /// # Arguments
  ///
  /// * `now` - Current Unix timestamp in seconds
  /// * `idle_ttl` - Maximum idle time in seconds before passivation
  fn passivate_idle(&mut self, now: u64, idle_ttl: u64) {
    let _ = (now, idle_ttl);
  }

  /// Drains pending virtual actor events.
  fn drain_events(&mut self) -> Vec<VirtualActorEvent> {
    Vec::new()
  }

  /// Drains pending PID cache events.
  fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    Vec::new()
  }
}
