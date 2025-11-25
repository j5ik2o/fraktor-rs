//! Core cluster state holder wiring dependencies and configuration.

#[cfg(test)]
mod tests;

use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ActivatedKind, ClusterError, ClusterEvent, ClusterExtensionConfig, ClusterMetrics, ClusterMetricsSnapshot,
  ClusterProvider, ClusterPubSub, ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError, KindRegistry,
  MetricsError, PidCache, StartupMode,
};

/// Aggregates configuration and shared dependencies for cluster runtime flows.
pub struct ClusterCore<TB: RuntimeToolbox + 'static> {
  provider:            ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  event_stream:        ArcShared<EventStreamGeneric<TB>>,
  gossiper:            ArcShared<ToolboxMutex<Box<dyn Gossiper>, TB>>,
  pub_sub:             ArcShared<ToolboxMutex<Box<dyn ClusterPubSub>, TB>>,
  startup_state:       ToolboxMutex<ClusterStartupState, TB>,
  metrics_enabled:     bool,
  kind_registry:       KindRegistry,
  identity_lookup:     ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>>,
  virtual_actor_count: i64,
  mode:                Option<StartupMode>,
  metrics:             Option<ClusterMetrics>,
  blocked_members:     Vec<String>,
  member_count:        usize,
  pid_cache:           Option<PidCache>,
  last_topology_hash:  Option<u64>,
}

impl<TB: RuntimeToolbox + 'static> ClusterCore<TB> {
  /// Builds a new cluster core from the provided dependencies.
  #[must_use]
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    config: &ClusterExtensionConfig,
    provider: ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    gossiper: ArcShared<ToolboxMutex<Box<dyn Gossiper>, TB>>,
    pubsub: ArcShared<ToolboxMutex<Box<dyn ClusterPubSub>, TB>>,
    kind_registry: KindRegistry,
    identity_lookup: ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>>,
  ) -> Self {
    let advertised_address = config.advertised_address().to_string();
    let startup_state = ClusterStartupState { address: advertised_address };
    let metrics_enabled = config.metrics_enabled();
    let virtual_actor_count = kind_registry.virtual_actor_count();
    let metrics: Option<ClusterMetrics> = if metrics_enabled { Some(ClusterMetrics::new()) } else { None };
    Self {
      provider,
      block_list_provider,
      event_stream,
      gossiper,
      pub_sub: pubsub,
      startup_state: <TB::MutexFamily as SyncMutexFamily>::create(startup_state),
      metrics_enabled,
      kind_registry,
      identity_lookup,
      virtual_actor_count,
      mode: None,
      metrics,
      blocked_members: Vec::new(),
      member_count: 0,
      pid_cache: None,
      last_topology_hash: None,
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

  /// Initializes kinds and sets up identity lookup in member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_member_kinds(&mut self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.kind_registry.register_all(kinds);
    self.virtual_actor_count = self.kind_registry.virtual_actor_count();
    let snapshot = self.kind_registry.all();
    self.identity_lookup.lock().setup_member(&snapshot)?;
    Ok(())
  }

  /// Initializes kinds and sets up identity lookup in client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_client_kinds(&mut self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.kind_registry.register_all(kinds);
    self.virtual_actor_count = self.kind_registry.virtual_actor_count();
    let snapshot = self.kind_registry.all();
    self.identity_lookup.lock().setup_client(&snapshot)?;
    Ok(())
  }

  /// Returns the aggregated virtual actor count.
  #[must_use]
  pub const fn virtual_actor_count(&self) -> i64 {
    self.virtual_actor_count
  }

  /// Returns the cached blocked members retrieved from the provider.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn blocked_members(&self) -> &[String] {
    &self.blocked_members
  }

  /// Installs a PID cache used for topology-driven invalidation (tests/core wiring).
  pub fn set_pid_cache(&mut self, cache: PidCache) {
    self.pid_cache = Some(cache);
  }

  /// Returns collected metrics snapshot if metrics are enabled.
  ///
  /// # Errors
  ///
  /// Returns [`MetricsError::Disabled`] if metrics collection is not enabled.
  pub const fn metrics(&self) -> Result<ClusterMetricsSnapshot, MetricsError> {
    match &self.metrics {
      | Some(metrics) => Ok(metrics.snapshot()),
      | None => Err(MetricsError::Disabled),
    }
  }

  /// Starts the cluster in member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    self.refresh_blocked_members();

    self.pub_sub.lock().start().map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode: StartupMode::Member,
        reason,
      });
      error
    })?;

    self.gossiper.lock().start().map_err(|reason| {
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode:    StartupMode::Member,
        reason:  reason.to_string(),
      });
      ClusterError::Gossip(reason)
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.lock().start_member();
    match provider_result {
      | Ok(()) => {
        self.mode = Some(StartupMode::Member);
        self.member_count = 1;
        self.update_metrics(self.member_count, self.virtual_actor_count);
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
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub or provider startup fails.
  pub fn start_client(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    self.refresh_blocked_members();

    self.pub_sub.lock().start().map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode: StartupMode::Client,
        reason,
      });
      error
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.lock().start_client();
    match provider_result {
      | Ok(()) => {
        self.mode = Some(StartupMode::Client);
        self.member_count = 1;
        self.update_metrics(self.member_count, self.virtual_actor_count);
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
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider shutdown fails.
  pub fn shutdown(&mut self, graceful: bool) -> Result<(), ClusterError> {
    let address = self.startup_address();
    let mode = self.mode.unwrap_or(StartupMode::Member);
    self.pub_sub.lock().stop().map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::ShutdownFailed { address: address.clone(), mode, reason });
      error
    })?;

    self.gossiper.lock().stop().map_err(|reason| {
      self.publish_cluster_event(ClusterEvent::ShutdownFailed {
        address: address.clone(),
        mode,
        reason: reason.to_string(),
      });
      ClusterError::Gossip(reason)
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.lock().shutdown(graceful);
    match provider_result {
      | Ok(()) => {
        self.virtual_actor_count = 0;
        self.member_count = 0;
        self.update_metrics(self.member_count, 0);
        self.blocked_members.clear();
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

  const fn update_metrics(&mut self, members: usize, virtual_actors: i64) {
    if let Some(metrics) = self.metrics.as_mut() {
      metrics.update_members(members);
      metrics.update_virtual_actors(virtual_actors);
    }
  }

  fn refresh_blocked_members(&mut self) {
    self.blocked_members = self.block_list_provider.blocked_members();
  }

  /// Applies a topology update and returns the event to be published.
  ///
  /// This method applies the topology internally but does NOT publish the event.
  /// The caller is responsible for publishing the returned event after releasing
  /// any locks to avoid deadlocks with EventStream subscribers.
  ///
  /// Returns `Some(ClusterEvent)` if the topology was applied (new hash),
  /// or `None` if the topology was a duplicate.
  #[must_use]
  pub fn apply_topology_for_external(&mut self, topology: &ClusterTopology) -> Option<ClusterEvent> {
    if self.apply_topology_internal(topology) {
      Some(ClusterEvent::TopologyUpdated {
        topology: topology.clone(),
        joined:   topology.joined().clone(),
        left:     topology.left().clone(),
        blocked:  self.blocked_members.clone(),
      })
    } else {
      None
    }
  }

  /// Applies a topology update, emitting a cluster event and updating metrics.
  ///
  /// Use this method when receiving topology updates from providers directly.
  /// For updates received via EventStream, use [`apply_topology`] instead to avoid
  /// re-publishing the event.
  ///
  /// **Warning**: This method publishes to EventStream while holding `&mut self`.
  /// If called from a context where a lock is held, consider using
  /// [`apply_topology_for_external`] instead to avoid deadlocks.
  pub fn on_topology(&mut self, topology: &ClusterTopology) {
    if self.apply_topology_internal(topology) {
      let event = ClusterEvent::TopologyUpdated {
        topology: topology.clone(),
        joined:   topology.joined().clone(),
        left:     topology.left().clone(),
        blocked:  self.blocked_members.clone(),
      };
      self.publish_cluster_event(event);
    }
  }

  /// Applies a topology update without publishing an event.
  ///
  /// Use this method when the topology update was already received via EventStream
  /// to avoid re-publishing and causing infinite loops.
  pub fn apply_topology(&mut self, topology: &ClusterTopology) {
    self.apply_topology_internal(topology);
  }

  /// Internal helper that applies topology and returns whether the update was applied.
  fn apply_topology_internal(&mut self, topology: &ClusterTopology) -> bool {
    if self.last_topology_hash == Some(topology.hash()) {
      return false;
    }
    self.last_topology_hash = Some(topology.hash());
    self.refresh_blocked_members();

    // Adjust member count using joined/left delta (saturating at zero).
    let joined = topology.joined().len();
    let left = topology.left().len();
    self.member_count = self.member_count.saturating_add(joined).saturating_sub(left);
    self.update_metrics(self.member_count, self.virtual_actor_count);

    if let Some(cache) = self.pid_cache.as_mut() {
      for authority in topology.left() {
        cache.invalidate_authority(authority);
      }
    }

    // IdentityLookup に離脱メンバーを伝播
    // 注: update_topology は完全なメンバーリストを必要とするため、
    // ClusterTopology のデルタ情報からは on_member_left のみを呼び出す
    {
      let mut identity_guard = self.identity_lookup.lock();
      for authority in topology.left() {
        identity_guard.on_member_left(authority);
      }
    }

    true
  }
}

#[derive(Clone)]
struct ClusterStartupState {
  address: String,
}
