//! Core cluster state holder wiring dependencies and configuration.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use alloc::string::ToString;

use fraktor_actor_rs::core::event_stream::{EventStreamEvent, EventStreamGeneric};
use fraktor_actor_rs::core::messaging::AnyMessageGeneric;
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ActivatedKind, ClusterError, ClusterEvent, ClusterExtensionConfig, ClusterProvider, IdentityLookup, IdentitySetupError,
  KindRegistry, StartupMode,
};

/// Aggregates configuration and shared dependencies for cluster runtime flows.
pub struct ClusterCore<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider:            ArcShared<dyn ClusterProvider>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  event_stream:        ArcShared<EventStreamGeneric<TB>>,
  startup_state:       ToolboxMutex<ClusterStartupState, TB>,
  metrics_enabled:     bool,
  kind_registry:       KindRegistry,
  identity_lookup:     ArcShared<dyn IdentityLookup>,
  virtual_actor_count: i64,
  mode:                Option<StartupMode>,
}

impl<TB: RuntimeToolbox + 'static> ClusterCore<TB> {
  /// Builds a new cluster core from the provided dependencies.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: ArcShared<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    kind_registry: KindRegistry,
    identity_lookup: ArcShared<dyn IdentityLookup>,
  ) -> Self {
    let advertised_address = config.advertised_address().to_string();
    let startup_state = ClusterStartupState { address: advertised_address };
    let metrics_enabled = config.metrics_enabled();
    let virtual_actor_count = kind_registry.virtual_actor_count();
    Self {
      config,
      provider,
      block_list_provider,
      event_stream,
      startup_state: <TB::MutexFamily as SyncMutexFamily>::create(startup_state),
      metrics_enabled,
      kind_registry,
      identity_lookup,
      virtual_actor_count,
      mode: None,
    }
  }

  /// Returns whether metrics collection is enabled.
  #[must_use]
  pub const fn metrics_enabled(&self) -> bool {
    self.metrics_enabled
  }

  /// Returns the advertised address shared by member and client modes.
  #[must_use]
  pub(crate) fn startup_address(&self) -> String {
    self.startup_state.lock().address.clone()
  }

  /// Returns the configuration captured at construction time.
  #[must_use]
  pub(crate) fn config(&self) -> &ClusterExtensionConfig {
    &self.config
  }

  /// Exposes the injected provider for internal callers.
  #[must_use]
  pub(crate) fn provider(&self) -> &ArcShared<dyn ClusterProvider> {
    &self.provider
  }

  /// Exposes the injected block list provider for internal callers.
  #[must_use]
  pub(crate) fn block_list_provider(&self) -> &ArcShared<dyn BlockListProvider> {
    &self.block_list_provider
  }

  /// Exposes the event stream handle.
  #[must_use]
  pub(crate) fn event_stream(&self) -> &ArcShared<EventStreamGeneric<TB>> {
    &self.event_stream
  }

  /// Initializes kinds and sets up identity lookup in member mode.
  pub fn setup_member_kinds(&mut self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.kind_registry.register_all(kinds);
    self.virtual_actor_count = self.kind_registry.virtual_actor_count();
    let snapshot = self.kind_registry.all();
    self.identity_lookup.setup_member(&snapshot)?;
    Ok(())
  }

  /// Initializes kinds and sets up identity lookup in client mode.
  pub fn setup_client_kinds(&mut self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.kind_registry.register_all(kinds);
    self.virtual_actor_count = self.kind_registry.virtual_actor_count();
    let snapshot = self.kind_registry.all();
    self.identity_lookup.setup_client(&snapshot)?;
    Ok(())
  }

  /// Returns the aggregated virtual actor count.
  #[must_use]
  pub const fn virtual_actor_count(&self) -> i64 {
    self.virtual_actor_count
  }

  /// Starts the cluster in member mode.
  pub fn start_member(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    match self.provider.start_member() {
      | Ok(()) => {
        self.mode = Some(StartupMode::Member);
        self.publish_cluster_event(ClusterEvent::Startup { address, mode: StartupMode::Member });
        Ok(())
      },
      | Err(error) => {
        let reason = error.reason().to_string();
        self.publish_cluster_event(ClusterEvent::StartupFailed { address, mode: StartupMode::Member, reason });
        Err(ClusterError::from(error))
      },
    }
  }

  /// Starts the cluster in client mode.
  pub fn start_client(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    match self.provider.start_client() {
      | Ok(()) => {
        self.mode = Some(StartupMode::Client);
        self.publish_cluster_event(ClusterEvent::Startup { address, mode: StartupMode::Client });
        Ok(())
      },
      | Err(error) => {
        let reason = error.reason().to_string();
        self.publish_cluster_event(ClusterEvent::StartupFailed { address, mode: StartupMode::Client, reason });
        Err(ClusterError::from(error))
      },
    }
  }

  /// Shuts down the cluster and resets metrics.
  pub fn shutdown(&mut self, graceful: bool) -> Result<(), ClusterError> {
    let address = self.startup_address();
    let mode = self.mode.unwrap_or(StartupMode::Member);
    match self.provider.shutdown(graceful) {
      | Ok(()) => {
        self.virtual_actor_count = 0;
        self.mode = None;
        self.publish_cluster_event(ClusterEvent::Shutdown { address, mode });
        Ok(())
      },
      | Err(error) => {
        let reason = error.reason().to_string();
        self.publish_cluster_event(ClusterEvent::ShutdownFailed { address, mode, reason });
        Err(ClusterError::from(error))
      },
    }
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }
}

#[derive(Clone)]
struct ClusterStartupState {
  address: String,
}
