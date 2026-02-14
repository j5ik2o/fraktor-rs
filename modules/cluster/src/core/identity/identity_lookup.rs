//! Identity lookup abstraction for cluster modes.

use alloc::{string::String, vec::Vec};

use super::{identity_setup_error::IdentitySetupError, lookup_error::LookupError, pid_cache_event::PidCacheEvent};
use crate::core::{
  grain::GrainKey,
  placement::{ActivatedKind, PlacementEvent, PlacementResolution},
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

  /// Resolves placement for a grain key.
  ///
  /// Returns `Ok(resolution)` when placement can be resolved, or
  /// `Err` when resolution failed or is pending.
  ///
  /// Note: This method uses `&mut self` because it may update the cache
  /// and create new activations as side effects.
  ///
  /// # Errors
  ///
  /// Returns an error when placement resolution fails or is pending.
  ///
  /// # Arguments
  ///
  /// * `key` - The grain key to resolve
  /// * `now` - Current Unix timestamp in seconds for TTL calculation
  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let _ = (key, now);
    Err(LookupError::NotReady)
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

  /// Drains pending placement events.
  fn drain_events(&mut self) -> Vec<PlacementEvent> {
    Vec::new()
  }

  /// Drains pending PID cache events.
  fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    Vec::new()
  }
}
