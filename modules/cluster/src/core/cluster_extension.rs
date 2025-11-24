//! Cluster extension wiring for actor systems.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ActivatedKind, ClusterCore, ClusterError, ClusterEvent, ClusterMetricsSnapshot, ClusterTopology, IdentitySetupError,
  MetricsError,
};

/// Internal subscriber that applies topology updates to ClusterCore.
struct ClusterTopologySubscriber<TB: RuntimeToolbox + 'static> {
  core: ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> ClusterTopologySubscriber<TB> {
  const fn new(core: ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>) -> Self {
    Self { core }
  }
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for ClusterTopologySubscriber<TB> {
  fn on_event(&self, event: &EventStreamEvent<TB>) {
    // cluster 拡張イベントの TopologyUpdated のみを処理
    // （既に EventStream 経由で受信したイベントなので再 publish しない）
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(ClusterEvent::TopologyUpdated { topology, .. }) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.core.lock().apply_topology(topology);
        }
      }
    }
  }
}

/// Cluster extension registered into `ActorSystemGeneric`.
pub struct ClusterExtensionGeneric<TB: RuntimeToolbox + 'static> {
  core:         ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>,
  event_stream: ArcShared<EventStreamGeneric<TB>>,
  subscription: ToolboxMutex<Option<EventStreamSubscriptionGeneric<TB>>, TB>,
  _system:      ArcShared<ActorSystemGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionGeneric<TB> {
  /// Creates the extension from injected dependencies.
  #[must_use]
  pub fn new(system: ArcShared<ActorSystemGeneric<TB>>, core: ClusterCore<TB>) -> Self {
    let event_stream = system.event_stream();
    let locked = <TB::MutexFamily as SyncMutexFamily>::create(core);
    let subscription = <TB::MutexFamily as SyncMutexFamily>::create(None);
    Self { core: ArcShared::new(locked), event_stream, subscription, _system: system }
  }

  /// Subscribes to the event stream for topology updates.
  fn subscribe_topology_events(&self) {
    // 既に購読中なら何もしない
    if self.subscription.lock().is_some() {
      return;
    }

    // ClusterCore への共有参照を持つ subscriber を作成
    let subscriber = ClusterTopologySubscriber::new(self.core.clone());
    let subscriber_arc: ArcShared<dyn EventStreamSubscriber<TB>> = ArcShared::new(subscriber);
    let sub = EventStreamGeneric::subscribe_arc(&self.event_stream, &subscriber_arc);
    *self.subscription.lock() = Some(sub);
  }

  /// Starts member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&self) -> Result<(), ClusterError> {
    let result = self.core.lock().start_member();
    if result.is_ok() {
      self.subscribe_topology_events();
    }
    result
  }

  /// Starts client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub or provider startup fails.
  pub fn start_client(&self) -> Result<(), ClusterError> {
    let result = self.core.lock().start_client();
    if result.is_ok() {
      self.subscribe_topology_events();
    }
    result
  }

  /// Graceful/forced shutdown.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider shutdown fails.
  pub fn shutdown(&self, graceful: bool) -> Result<(), ClusterError> {
    // 購読を解除
    *self.subscription.lock() = None;
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
