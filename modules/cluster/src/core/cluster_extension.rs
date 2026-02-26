//! Cluster extension wiring for actor systems.

#[cfg(test)]
mod tests;

use alloc::{format, string::String, vec::Vec};

use fraktor_actor_rs::core::{
  event::stream::{
    EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
    subscriber_handle,
  },
  messaging::AnyMessageGeneric,
  system::{ActorSystemGeneric, ActorSystemWeakGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  ClusterCore, ClusterError, ClusterEvent, ClusterMetricsSnapshot, MetricsError, TopologyUpdate,
  grain::{GrainMetrics, GrainMetricsSharedGeneric, GrainMetricsSnapshot},
  identity::IdentitySetupError,
  membership::NodeStatus,
  placement::ActivatedKind,
};

const CLUSTER_EVENT_STREAM_NAME: &str = "cluster";

/// Internal subscriber that applies topology updates to ClusterCore.
struct ClusterTopologySubscriber<TB: RuntimeToolbox + 'static> {
  core:         ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>,
  event_stream: EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ClusterTopologySubscriber<TB> {
  const fn new(core: ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>, event_stream: EventStreamSharedGeneric<TB>) -> Self {
    Self { core, event_stream }
  }
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for ClusterTopologySubscriber<TB> {
  fn on_event(&mut self, event: &EventStreamEvent<TB>) {
    // cluster 拡張イベントの TopologyUpdated のみを処理
    // （既に EventStream 経由で受信したイベントなので再 publish しない）
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(ClusterEvent::TopologyUpdated { update }) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      let result = self.core.lock().try_apply_topology(update);
      if let Err(error) = result {
        let reason = format!("{error:?}");
        let failed = ClusterEvent::TopologyApplyFailed { reason, observed_at: update.observed_at };
        let payload = AnyMessageGeneric::new(failed);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        self.event_stream.publish(&extension_event);
      }
    }
  }
}

struct MemberStatusSubscriberState {
  subscription_id:       Option<u64>,
  unsubscribe_requested: bool,
}

impl MemberStatusSubscriberState {
  const fn new() -> Self {
    Self { subscription_id: None, unsubscribe_requested: false }
  }
}

struct MemberStatusCallbackState<F> {
  callback: F,
  fired:    bool,
}

impl<F> MemberStatusCallbackState<F> {
  const fn new(callback: F) -> Self {
    Self { callback, fired: false }
  }
}

fn trigger_member_status_callback<TB, F>(
  callback_state: &ArcShared<ToolboxMutex<MemberStatusCallbackState<F>, TB>>,
  node_id: &str,
  authority: &str,
) -> bool
where
  TB: RuntimeToolbox + 'static,
  F: FnMut(&str, &str) + Send + Sync + 'static, {
  let mut state = callback_state.lock();
  if state.fired {
    return false;
  }
  state.fired = true;
  (state.callback)(node_id, authority);
  true
}

#[derive(Clone)]
struct SelfMemberStatus {
  node_id:   String,
  authority: String,
  status:    NodeStatus,
}

struct SelfMemberStatusTrackerSubscriber<TB: RuntimeToolbox + 'static> {
  self_address: String,
  self_status:  ArcShared<ToolboxMutex<Option<SelfMemberStatus>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> SelfMemberStatusTrackerSubscriber<TB> {
  const fn new(self_address: String, self_status: ArcShared<ToolboxMutex<Option<SelfMemberStatus>, TB>>) -> Self {
    Self { self_address, self_status }
  }
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for SelfMemberStatusTrackerSubscriber<TB> {
  fn on_event(&mut self, event: &EventStreamEvent<TB>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(ClusterEvent::MemberStatusChanged { node_id, authority, to, .. }) =
        payload.payload().downcast_ref::<ClusterEvent>()
      && authority == &self.self_address
    {
      let status = SelfMemberStatus { node_id: node_id.clone(), authority: authority.clone(), status: *to };
      *self.self_status.lock() = Some(status);
    }
  }
}

struct MemberStatusSubscriber<TB: RuntimeToolbox + 'static, F: FnMut(&str, &str) + Send + Sync + 'static> {
  target:         NodeStatus,
  self_address:   String,
  callback_state: ArcShared<ToolboxMutex<MemberStatusCallbackState<F>, TB>>,
  state:          ArcShared<ToolboxMutex<MemberStatusSubscriberState, TB>>,
  event_stream:   EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static, F: FnMut(&str, &str) + Send + Sync + 'static> MemberStatusSubscriber<TB, F> {
  const fn new(
    target: NodeStatus,
    self_address: String,
    callback_state: ArcShared<ToolboxMutex<MemberStatusCallbackState<F>, TB>>,
    state: ArcShared<ToolboxMutex<MemberStatusSubscriberState, TB>>,
    event_stream: EventStreamSharedGeneric<TB>,
  ) -> Self {
    Self { target, self_address, callback_state, state, event_stream }
  }
}

impl<TB, F> EventStreamSubscriber<TB> for MemberStatusSubscriber<TB, F>
where
  TB: RuntimeToolbox + 'static,
  F: FnMut(&str, &str) + Send + Sync + 'static,
{
  fn on_event(&mut self, event: &EventStreamEvent<TB>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
      && let ClusterEvent::MemberStatusChanged { node_id, authority, to, .. } = cluster_event
      && authority == &self.self_address
      && *to == self.target
      && trigger_member_status_callback::<TB, F>(&self.callback_state, node_id, authority)
    {
      let subscription_id = {
        let mut state = self.state.lock();
        state.unsubscribe_requested = true;
        state.subscription_id
      };
      if let Some(id) = subscription_id {
        self.event_stream.unsubscribe(id);
      }
    }
  }
}

struct NoopMemberStatusSubscriber;

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for NoopMemberStatusSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent<TB>) {}
}

/// Cluster extension registered into `ActorSystemGeneric`.
pub struct ClusterExtensionGeneric<TB: RuntimeToolbox + 'static> {
  core: ArcShared<ToolboxMutex<ClusterCore<TB>, TB>>,
  event_stream: EventStreamSharedGeneric<TB>,
  grain_metrics: Option<GrainMetricsSharedGeneric<TB>>,
  subscription: ToolboxMutex<Option<EventStreamSubscriptionGeneric<TB>>, TB>,
  terminated: ToolboxMutex<bool, TB>,
  self_member_status: ArcShared<ToolboxMutex<Option<SelfMemberStatus>, TB>>,
  _self_member_status_subscription: EventStreamSubscriptionGeneric<TB>,
  _system: ActorSystemWeakGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionGeneric<TB> {
  /// Creates the extension from injected dependencies.
  ///
  /// Uses a weak reference to the actor system to avoid circular references.
  #[must_use]
  pub fn new(system: &ActorSystemGeneric<TB>, core: ClusterCore<TB>) -> Self {
    let event_stream = system.event_stream();
    let self_address = core.startup_address();
    let grain_metrics =
      if core.metrics_enabled() { Some(GrainMetricsSharedGeneric::new(GrainMetrics::new())) } else { None };
    let self_member_status = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(None));
    let status_subscriber =
      subscriber_handle::<TB>(SelfMemberStatusTrackerSubscriber::<TB>::new(self_address, self_member_status.clone()));
    let self_member_status_subscription = event_stream.subscribe_no_replay(&status_subscriber);
    let locked = <TB::MutexFamily as SyncMutexFamily>::create(core);
    let subscription = <TB::MutexFamily as SyncMutexFamily>::create(None);
    let terminated = <TB::MutexFamily as SyncMutexFamily>::create(false);
    Self {
      core: ArcShared::new(locked),
      event_stream,
      grain_metrics,
      subscription,
      terminated,
      self_member_status,
      _self_member_status_subscription: self_member_status_subscription,
      _system: system.downgrade(),
    }
  }

  /// Returns the shared cluster core handle.
  #[must_use]
  pub(crate) fn core_shared(&self) -> ArcShared<ToolboxMutex<ClusterCore<TB>, TB>> {
    self.core.clone()
  }

  /// Returns the shared pub/sub handle.
  #[must_use]
  pub(crate) fn pub_sub_shared(&self) -> crate::core::pub_sub::ClusterPubSubShared<TB> {
    self.core.lock().pub_sub_shared()
  }

  /// Returns the shared grain metrics handle if enabled.
  #[must_use]
  pub(crate) fn grain_metrics_shared(&self) -> Option<GrainMetricsSharedGeneric<TB>> {
    self.grain_metrics.clone()
  }

  /// Subscribes to the event stream for topology updates.
  fn subscribe_topology_events(&self) {
    // 既に購読中なら何もしない
    if self.subscription.lock().is_some() {
      return;
    }

    // ClusterCore への共有参照を持つ subscriber を作成
    let subscriber: ClusterTopologySubscriber<TB> =
      ClusterTopologySubscriber::new(self.core.clone(), self.event_stream.clone());
    let subscriber_handle = subscriber_handle(subscriber);
    let sub = self.event_stream.subscribe(&subscriber_handle);
    *self.subscription.lock() = Some(sub);
  }

  /// Starts member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&self) -> Result<(), ClusterError> {
    *self.self_member_status.lock() = None;
    let result = self.core.lock().start_member();
    if result.is_ok() {
      *self.terminated.lock() = false;
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
    *self.self_member_status.lock() = None;
    let result = self.core.lock().start_client();
    if result.is_ok() {
      *self.terminated.lock() = false;
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
    let result = self.core.lock().shutdown(graceful);
    if result.is_ok() {
      *self.terminated.lock() = true;
    }
    result
  }

  /// Explicitly downs the provided member authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or the provider cannot process downing.
  pub fn down(&self, authority: &str) -> Result<(), ClusterError> {
    self.core.lock().down(authority)
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
  ///
  /// This method applies the topology and publishes the event to EventStream.
  /// The lock is released before publishing to avoid deadlocks with subscribers.
  pub fn on_topology(&self, update: &TopologyUpdate) {
    // ロックを保持したまま publish するとデッドロックするため、
    // イベントを取得してからロックを解放し、その後に publish する
    let result = { self.core.lock().try_apply_topology(update) };

    match result {
      | Ok(Some(event)) => {
        let payload = AnyMessageGeneric::new(event);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        self.event_stream.publish(&extension_event);
      },
      | Ok(None) => {},
      | Err(error) => {
        let reason = format!("{error:?}");
        let failed = ClusterEvent::TopologyApplyFailed { reason, observed_at: update.observed_at };
        let payload = AnyMessageGeneric::new(failed);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        self.event_stream.publish(&extension_event);
      },
    }
  }

  /// Registers a callback invoked when a member reaches `Up` status.
  #[must_use]
  pub fn register_on_member_up<F>(&self, callback: F) -> EventStreamSubscriptionGeneric<TB>
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    self.register_on_member_status(NodeStatus::Up, callback)
  }

  /// Registers a callback invoked when a member reaches `Removed` status.
  #[must_use]
  pub fn register_on_member_removed<F>(&self, callback: F) -> EventStreamSubscriptionGeneric<TB>
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    if *self.terminated.lock() {
      let self_address = self.core.lock().startup_address();
      let node_id = {
        let current = self.self_member_status.lock();
        if let Some(status) = current.as_ref() { status.node_id.clone() } else { self_address.clone() }
      };
      let mut immediate = callback;
      immediate(&node_id, &self_address);
      return self.already_unsubscribed_subscription();
    }
    self.register_on_member_status(NodeStatus::Removed, callback)
  }

  /// Returns metrics snapshot if enabled.
  ///
  /// # Errors
  ///
  /// Returns [`MetricsError::Disabled`] if metrics collection is not enabled.
  pub fn metrics(&self) -> Result<ClusterMetricsSnapshot, MetricsError> {
    self.core.lock().metrics()
  }

  /// Returns grain metrics snapshot if enabled.
  ///
  /// # Errors
  ///
  /// Returns [`MetricsError::Disabled`] if metrics collection is not enabled.
  pub fn grain_metrics(&self) -> Result<GrainMetricsSnapshot, MetricsError> {
    match &self.grain_metrics {
      | Some(metrics) => Ok(metrics.with_read(|inner| inner.snapshot())),
      | None => Err(MetricsError::Disabled),
    }
  }

  /// Returns virtual actor count.
  pub fn virtual_actor_count(&self) -> i64 {
    self.core.lock().virtual_actor_count()
  }

  /// Returns blocked members cache.
  pub fn blocked_members(&self) -> Vec<String> {
    self.core.lock().blocked_members().to_vec()
  }

  fn register_on_member_status<F>(&self, target: NodeStatus, callback: F) -> EventStreamSubscriptionGeneric<TB>
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    let self_address = self.core.lock().startup_address();
    let state = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(MemberStatusSubscriberState::new()));
    let callback_state =
      ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(MemberStatusCallbackState::new(callback)));
    let subscriber = subscriber_handle::<TB>(MemberStatusSubscriber::new(
      target,
      self_address.clone(),
      callback_state.clone(),
      state.clone(),
      self.event_stream.clone(),
    ));
    let subscription = self.event_stream.subscribe_no_replay(&subscriber);
    let subscription_id = subscription.id();
    {
      let mut guard = state.lock();
      guard.subscription_id = Some(subscription_id);
    }
    if let Some(current) = self.self_member_status.lock().clone()
      && current.authority == self_address.as_str()
      && current.status == target
      && trigger_member_status_callback::<TB, F>(&callback_state, &current.node_id, &current.authority)
    {
      state.lock().unsubscribe_requested = true;
    }
    let unsubscribe_now = state.lock().unsubscribe_requested;
    if unsubscribe_now {
      self.event_stream.unsubscribe(subscription_id);
    }
    subscription
  }

  fn already_unsubscribed_subscription(&self) -> EventStreamSubscriptionGeneric<TB> {
    let subscriber = subscriber_handle::<TB>(NoopMemberStatusSubscriber);
    let subscription = self.event_stream.subscribe(&subscriber);
    self.event_stream.unsubscribe(subscription.id());
    subscription
  }
}

impl<TB: RuntimeToolbox + 'static> fraktor_actor_rs::core::extension::Extension<TB> for ClusterExtensionGeneric<TB> {}
