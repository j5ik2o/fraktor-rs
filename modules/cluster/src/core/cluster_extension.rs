//! Cluster extension wiring for actor systems.

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::system::ActorSystemGeneric;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ActivatedKind, ClusterCore, ClusterError, ClusterMetricsSnapshot, ClusterTopology, IdentitySetupError, MetricsError,
};

/// Cluster extension registered into `ActorSystemGeneric`.
pub struct ClusterExtensionGeneric<TB: RuntimeToolbox + 'static> {
  core:    ToolboxMutex<ClusterCore<TB>, TB>,
  _system: ArcShared<ActorSystemGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionGeneric<TB> {
  /// Creates the extension from injected dependencies.
  #[must_use]
  pub fn new(system: ArcShared<ActorSystemGeneric<TB>>, core: ClusterCore<TB>) -> Self {
    let locked = <TB::MutexFamily as SyncMutexFamily>::create(core);
    Self { core: locked, _system: system }
  }

  /// Starts member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&self) -> Result<(), ClusterError> {
    self.core.lock().start_member()
  }

  /// Starts client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub or provider startup fails.
  pub fn start_client(&self) -> Result<(), ClusterError> {
    self.core.lock().start_client()
  }

  /// Graceful/forced shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider shutdown fails.
  pub fn shutdown(&self, graceful: bool) -> Result<(), ClusterError> {
    self.core.lock().shutdown(graceful)
  }

  /// Registers kinds for member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_member_kinds(&self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.core.lock().setup_member_kinds(kinds)
  }

  /// Registers kinds for client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_client_kinds(&self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.core.lock().setup_client_kinds(kinds)
  }

  /// Applies topology updates.
  pub fn on_topology(&self, topology: &ClusterTopology) {
    self.core.lock().on_topology(topology);
  }

  /// Returns metrics snapshot if enabled.
  ///
  /// # Errors
  ///
  /// Returns [`MetricsError::Disabled`] if metrics collection is not enabled.
  pub fn metrics(&self) -> Result<ClusterMetricsSnapshot, MetricsError> {
    self.core.lock().metrics()
  }

  /// Returns virtual actor count.
  pub fn virtual_actor_count(&self) -> i64 {
    self.core.lock().virtual_actor_count()
  }

  /// Returns blocked members cache.
  pub fn blocked_members(&self) -> Vec<String> {
    self.core.lock().blocked_members().to_vec()
  }
}

impl<TB: RuntimeToolbox + 'static> fraktor_actor_rs::core::extension::Extension<TB> for ClusterExtensionGeneric<TB> {}
