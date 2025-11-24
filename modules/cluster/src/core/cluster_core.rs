//! Core cluster state holder wiring dependencies and configuration.

#[cfg(test)]
mod tests;

use alloc::string::String;

use fraktor_actor_rs::core::event_stream::EventStreamGeneric;
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{ClusterExtensionConfig, ClusterProvider};

/// Aggregates configuration and shared dependencies for cluster runtime flows.
pub struct ClusterCore<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider:            ArcShared<dyn ClusterProvider>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  event_stream:        ArcShared<EventStreamGeneric<TB>>,
  startup_state:       ToolboxMutex<ClusterStartupState, TB>,
  metrics_enabled:     bool,
}

impl<TB: RuntimeToolbox + 'static> ClusterCore<TB> {
  /// Builds a new cluster core from the provided dependencies.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: ArcShared<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    event_stream: ArcShared<EventStreamGeneric<TB>>,
  ) -> Self {
    let advertised_address = config.advertised_address().to_string();
    let startup_state = ClusterStartupState { address: advertised_address };
    let metrics_enabled = config.metrics_enabled();
    Self {
      config,
      provider,
      block_list_provider,
      event_stream,
      startup_state: <TB::MutexFamily as SyncMutexFamily>::create(startup_state),
      metrics_enabled,
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
}

#[derive(Clone)]
struct ClusterStartupState {
  address: String,
}
