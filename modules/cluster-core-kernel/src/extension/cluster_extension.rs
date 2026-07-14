//! Cluster extension wiring for actor systems.

#[cfg(test)]
#[path = "cluster_extension_test.rs"]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::ActorRef,
    error::SendError,
    messaging::AnyMessage,
    props::Props,
    scheduler::{ExecutionBatch, SchedulerCommand, SchedulerHandle, SchedulerRunnable},
  },
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscription, subscriber_handle,
  },
  system::{ActorSystem, ActorSystemWeak},
};
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

use super::grain_idle_passivation_actor::GrainIdlePassivationActor;
use crate::{
  ClusterCore, ClusterError, ClusterEvent, ClusterMetricsSnapshot, MetricsError, StartupMode, TopologyUpdate,
  activation::{ActivatedKind, IdentitySetupError, PlacementEvent},
  grain::{
    GRAIN_EVENT_STREAM_NAME, GrainEvent, GrainMetrics, GrainMetricsShared, GrainMetricsSnapshot, GrainReadinessSnapshot,
  },
  membership::NodeStatus,
  pub_sub::ClusterPubSubShared,
};

const CLUSTER_EVENT_STREAM_NAME: &str = "cluster";

fn report_idle_passivation_delivery_failure(error: &SendError) {
  tracing::warn!(?error, "failed to enqueue Grain idle passivation sweep");
}

/// Internal subscriber that applies topology updates to ClusterCore.
struct ClusterTopologySubscriber {
  core: SharedLock<ClusterCore>,
  event_stream: EventStreamShared,
  self_address: String,
  self_status: SharedLock<Option<SelfMemberStatus>>,
  self_identity: SharedLock<Option<SelfMemberIdentity>>,
  starting_identity: SharedLock<Option<SelfMemberIdentity>>,
  topology_absent_identities: SharedLock<Vec<SelfMemberIdentity>>,
}

impl ClusterTopologySubscriber {
  const fn new(
    core: SharedLock<ClusterCore>,
    event_stream: EventStreamShared,
    self_address: String,
    self_status: SharedLock<Option<SelfMemberStatus>>,
    self_identity: SharedLock<Option<SelfMemberIdentity>>,
    starting_identity: SharedLock<Option<SelfMemberIdentity>>,
    topology_absent_identities: SharedLock<Vec<SelfMemberIdentity>>,
  ) -> Self {
    Self { core, event_stream, self_address, self_status, self_identity, starting_identity, topology_absent_identities }
  }
}

impl EventStreamSubscriber for ClusterTopologySubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    // cluster 拡張イベントの TopologyUpdated のみを処理
    // （既に EventStream 経由で受信したイベントなので再 publish しない）
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(ClusterEvent::TopologyUpdated { update }) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      let result = self.core.with_lock(|core| core.try_apply_topology(update));
      if matches!(result.as_ref(), Ok(Some(_))) {
        clear_self_observation_if_absent(
          update,
          &self.self_address,
          &self.self_status,
          &self.self_identity,
          &self.starting_identity,
          &self.topology_absent_identities,
        );
      }
      if let Err(error) = result {
        let reason = format!("{error:?}");
        let failed = ClusterEvent::TopologyApplyFailed { reason, observed_at: update.observed_at };
        let payload = AnyMessage::new(failed);
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

fn trigger_member_status_callback<F>(
  callback_state: &SharedLock<MemberStatusCallbackState<F>>,
  node_id: &str,
  authority: &str,
) -> bool
where
  F: FnMut(&str, &str) + Send + Sync + 'static, {
  callback_state.with_lock(|state| {
    if state.fired {
      return false;
    }
    state.fired = true;
    (state.callback)(node_id, authority);
    true
  })
}

#[derive(Clone)]
struct SelfMemberStatus {
  node_id:   String,
  authority: String,
  status:    NodeStatus,
}

#[derive(Clone)]
struct SelfMemberIdentity {
  node_id:   String,
  authority: String,
}

fn remember_retired_identity(
  suppressed_retired_identities: &SharedLock<Vec<SelfMemberIdentity>>,
  identity: SelfMemberIdentity,
) {
  suppressed_retired_identities.with_lock(|retired_identities| {
    if retired_identities
      .iter()
      .any(|retired| retired.node_id == identity.node_id && retired.authority == identity.authority)
    {
      return;
    }
    retired_identities.push(identity);
  });
}

fn remember_unique_identity(identities: &SharedLock<Vec<SelfMemberIdentity>>, identity: SelfMemberIdentity) {
  identities.with_lock(|identities| {
    if identities.iter().any(|current| current.node_id == identity.node_id && current.authority == identity.authority) {
      return;
    }
    identities.push(identity);
  });
}

fn identity_matches(left: &SelfMemberIdentity, right: &SelfMemberIdentity) -> bool {
  left.node_id == right.node_id && left.authority == right.authority
}

fn is_suppressed_retired_identity(
  suppressed_retired_identities: &SharedLock<Vec<SelfMemberIdentity>>,
  identity: &SelfMemberIdentity,
) -> bool {
  suppressed_retired_identities
    .with_lock(|retired_identities| retired_identities.iter().any(|retired| identity_matches(retired, identity)))
}

fn is_topology_absent_identity(
  topology_absent_identities: &SharedLock<Vec<SelfMemberIdentity>>,
  identity: &SelfMemberIdentity,
) -> bool {
  topology_absent_identities
    .with_lock(|absent_identities| absent_identities.iter().any(|absent| identity_matches(absent, identity)))
}

const fn is_rejoin_transition(from: NodeStatus, to: NodeStatus) -> bool {
  matches!((from, to), (NodeStatus::Removed | NodeStatus::Dead, NodeStatus::Joining))
}

fn can_accept_retired_self_status(
  identity: &SelfMemberIdentity,
  from: NodeStatus,
  to: NodeStatus,
  self_identity: &SharedLock<Option<SelfMemberIdentity>>,
  starting_identity: &SharedLock<Option<SelfMemberIdentity>>,
  start_in_progress: &SharedLock<bool>,
) -> bool {
  let current_identity = self_identity.with_lock(|self_identity| self_identity.clone());
  if current_identity.is_some() {
    return false;
  }
  let is_starting_identity = starting_identity.with_lock(|starting_identity| {
    starting_identity.as_ref().is_some_and(|starting| identity_matches(starting, identity))
  });
  if is_rejoin_transition(from, to) {
    return is_starting_identity;
  }
  is_starting_identity
    && start_in_progress.with_lock(|start_in_progress| *start_in_progress)
    && to != NodeStatus::Removed
}

fn retain_non_matching_identity(
  suppressed_retired_identities: &SharedLock<Vec<SelfMemberIdentity>>,
  identity: &SelfMemberIdentity,
) {
  suppressed_retired_identities.with_lock(|retired_identities| {
    retired_identities.retain(|retired| !identity_matches(retired, identity));
  });
}

fn clear_starting_identity_if_replaced(
  starting_identity: &SharedLock<Option<SelfMemberIdentity>>,
  identity: &SelfMemberIdentity,
) {
  starting_identity.with_lock(|starting_identity| {
    if starting_identity.as_ref().is_some_and(|starting| !identity_matches(starting, identity)) {
      *starting_identity = None;
    }
  });
}

#[derive(Clone)]
struct SelfMemberLifecycle {
  self_status:                   SharedLock<Option<SelfMemberStatus>>,
  self_identity:                 SharedLock<Option<SelfMemberIdentity>>,
  terminated:                    SharedLock<bool>,
  suppressed_retired_identities: SharedLock<Vec<SelfMemberIdentity>>,
  starting_identity:             SharedLock<Option<SelfMemberIdentity>>,
  start_in_progress:             SharedLock<bool>,
  topology_absent_identities:    SharedLock<Vec<SelfMemberIdentity>>,
}

impl SelfMemberLifecycle {
  const fn new(
    self_status: SharedLock<Option<SelfMemberStatus>>,
    self_identity: SharedLock<Option<SelfMemberIdentity>>,
    terminated: SharedLock<bool>,
    suppressed_retired_identities: SharedLock<Vec<SelfMemberIdentity>>,
    starting_identity: SharedLock<Option<SelfMemberIdentity>>,
    start_in_progress: SharedLock<bool>,
    topology_absent_identities: SharedLock<Vec<SelfMemberIdentity>>,
  ) -> Self {
    Self {
      self_status,
      self_identity,
      terminated,
      suppressed_retired_identities,
      starting_identity,
      start_in_progress,
      topology_absent_identities,
    }
  }
}

fn clear_self_observation_if_absent(
  update: &TopologyUpdate,
  self_address: &str,
  self_status: &SharedLock<Option<SelfMemberStatus>>,
  self_identity: &SharedLock<Option<SelfMemberIdentity>>,
  starting_identity: &SharedLock<Option<SelfMemberIdentity>>,
  topology_absent_identities: &SharedLock<Vec<SelfMemberIdentity>>,
) {
  if update.members.iter().any(|member| member == self_address) {
    return;
  }
  let previous_identity = self_identity.with_lock(|identity| identity.take());
  let previous_status = self_status.with_lock(|status| status.clone());
  starting_identity.with_lock(|identity| *identity = None);
  if !previous_status.as_ref().is_some_and(|status| status.status == NodeStatus::Removed) {
    self_status.with_lock(|status| *status = None);
  }
  if let Some(identity) = previous_identity {
    remember_unique_identity(topology_absent_identities, identity);
  }
}

struct SelfMemberStatusTrackerSubscriber {
  self_address:                  String,
  self_status:                   SharedLock<Option<SelfMemberStatus>>,
  self_identity:                 SharedLock<Option<SelfMemberIdentity>>,
  terminated:                    SharedLock<bool>,
  suppressed_retired_identities: SharedLock<Vec<SelfMemberIdentity>>,
  starting_identity:             SharedLock<Option<SelfMemberIdentity>>,
  start_in_progress:             SharedLock<bool>,
  topology_absent_identities:    SharedLock<Vec<SelfMemberIdentity>>,
}

impl SelfMemberStatusTrackerSubscriber {
  #[allow(clippy::too_many_arguments)]
  const fn new(
    self_address: String,
    self_status: SharedLock<Option<SelfMemberStatus>>,
    self_identity: SharedLock<Option<SelfMemberIdentity>>,
    terminated: SharedLock<bool>,
    suppressed_retired_identities: SharedLock<Vec<SelfMemberIdentity>>,
    starting_identity: SharedLock<Option<SelfMemberIdentity>>,
    start_in_progress: SharedLock<bool>,
    topology_absent_identities: SharedLock<Vec<SelfMemberIdentity>>,
  ) -> Self {
    Self {
      self_address,
      self_status,
      self_identity,
      terminated,
      suppressed_retired_identities,
      starting_identity,
      start_in_progress,
      topology_absent_identities,
    }
  }
}

impl EventStreamSubscriber for SelfMemberStatusTrackerSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(ClusterEvent::MemberStatusChanged { node_id, authority, from, to, .. }) =
        payload.payload().downcast_ref::<ClusterEvent>()
      && authority == &self.self_address
    {
      let is_terminated = self.terminated.with_lock(|terminated| *terminated);
      let identity = SelfMemberIdentity { node_id: node_id.clone(), authority: authority.clone() };
      let is_topology_absent = is_topology_absent_identity(&self.topology_absent_identities, &identity);
      let can_rejoin = is_rejoin_transition(*from, *to);
      if is_topology_absent && *to != NodeStatus::Removed && !can_rejoin {
        return;
      }
      let is_retired_identity = is_suppressed_retired_identity(&self.suppressed_retired_identities, &identity);
      let can_rejoin_with_same_identity = can_accept_retired_self_status(
        &identity,
        *from,
        *to,
        &self.self_identity,
        &self.starting_identity,
        &self.start_in_progress,
      );
      if is_retired_identity && !can_rejoin_with_same_identity {
        return;
      }
      if is_terminated && *to != NodeStatus::Removed {
        return;
      }
      let current_status = self.self_status.with_lock(|self_status| self_status.clone());
      if let Some(current) = self.self_identity.with_lock(|self_identity| self_identity.clone())
        && (current.node_id != identity.node_id || current.authority != identity.authority)
      {
        let current_is_retired = is_suppressed_retired_identity(&self.suppressed_retired_identities, &current);
        // 開始対象 identity の置換は start window 中に限る。完了後に starting_identity == current の間へ
        // 別 identity の遅延イベントが届いても self_member_identity を上書きさせない。
        let in_start_window = self.start_in_progress.with_lock(|start_in_progress| *start_in_progress);
        let can_replace_starting_identity = in_start_window
          && self.starting_identity.with_lock(|starting_identity| {
            starting_identity
              .as_ref()
              .is_some_and(|starting| identity_matches(starting, &current) && !is_retired_identity)
          });
        let incoming_is_starting_identity = self.starting_identity.with_lock(|starting_identity| {
          starting_identity.as_ref().is_some_and(|starting| identity_matches(starting, &identity))
        });
        // Removed / 退役済みの self を別 identity で置換できるのは、終端を巻き戻さない Removed イベント、
        // 正規の再参加遷移、開始対象 identity の 3 ケースに限る。古い incarnation の遅延 Up で
        // observed status を非終端へ巻き戻さない。
        let current_is_terminal =
          current_status.as_ref().is_some_and(|status| status.status == NodeStatus::Removed) || current_is_retired;
        let can_replace_removed_identity =
          current_is_terminal && (*to == NodeStatus::Removed || can_rejoin || incoming_is_starting_identity);
        if !can_replace_starting_identity && !can_replace_removed_identity {
          return;
        }
      }
      if let Some(current) = self.self_identity.with_lock(|self_identity| self_identity.clone())
        && identity_matches(&current, &identity)
        && current_status.as_ref().is_some_and(|status| status.status == NodeStatus::Removed)
        && *to != NodeStatus::Removed
        && !can_rejoin
      {
        remember_retired_identity(&self.suppressed_retired_identities, identity);
        return;
      }
      let status = SelfMemberStatus { node_id: node_id.clone(), authority: authority.clone(), status: *to };
      retain_non_matching_identity(&self.suppressed_retired_identities, &identity);
      if !is_topology_absent || *to != NodeStatus::Removed {
        retain_non_matching_identity(&self.topology_absent_identities, &identity);
      }
      clear_starting_identity_if_replaced(&self.starting_identity, &identity);
      self.self_status.with_lock(|self_status| *self_status = Some(status));
      self.self_identity.with_lock(|self_identity| *self_identity = Some(identity));
    }
  }
}

struct MemberStatusSubscriber<F: FnMut(&str, &str) + Send + Sync + 'static> {
  target:         NodeStatus,
  self_address:   String,
  callback_state: SharedLock<MemberStatusCallbackState<F>>,
  state:          SharedLock<MemberStatusSubscriberState>,
  event_stream:   EventStreamShared,
  lifecycle:      SelfMemberLifecycle,
}

impl<F: FnMut(&str, &str) + Send + Sync + 'static> MemberStatusSubscriber<F> {
  const fn new(
    target: NodeStatus,
    self_address: String,
    callback_state: SharedLock<MemberStatusCallbackState<F>>,
    state: SharedLock<MemberStatusSubscriberState>,
    event_stream: EventStreamShared,
    lifecycle: SelfMemberLifecycle,
  ) -> Self {
    Self { target, self_address, callback_state, state, event_stream, lifecycle }
  }
}

impl<F> EventStreamSubscriber for MemberStatusSubscriber<F>
where
  F: FnMut(&str, &str) + Send + Sync + 'static,
{
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == CLUSTER_EVENT_STREAM_NAME
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
      && let ClusterEvent::MemberStatusChanged { node_id, authority, from, to, .. } = cluster_event
      && authority == &self.self_address
      && *to == self.target
    {
      let identity = SelfMemberIdentity { node_id: node_id.clone(), authority: authority.clone() };
      if self.lifecycle.terminated.with_lock(|terminated| *terminated) && *to != NodeStatus::Removed {
        return;
      }
      let is_topology_absent = is_topology_absent_identity(&self.lifecycle.topology_absent_identities, &identity);
      let can_rejoin = is_rejoin_transition(*from, *to);
      if is_topology_absent && *to != NodeStatus::Removed && !can_rejoin {
        return;
      }
      let is_retired_identity =
        is_suppressed_retired_identity(&self.lifecycle.suppressed_retired_identities, &identity);
      let can_rejoin_with_same_identity = can_accept_retired_self_status(
        &identity,
        *from,
        *to,
        &self.lifecycle.self_identity,
        &self.lifecycle.starting_identity,
        &self.lifecycle.start_in_progress,
      );
      if is_retired_identity && !can_rejoin_with_same_identity {
        return;
      }
      let current_status = self.lifecycle.self_status.with_lock(|self_status| self_status.clone());
      if let Some(current) = self.lifecycle.self_identity.with_lock(|self_identity| self_identity.clone())
        && !identity_matches(&current, &identity)
      {
        let current_is_removed = current_status.as_ref().is_some_and(|status| status.status == NodeStatus::Removed);
        let current_is_retired =
          is_suppressed_retired_identity(&self.lifecycle.suppressed_retired_identities, &current);
        let incoming_is_starting_identity = self.lifecycle.starting_identity.with_lock(|starting_identity| {
          starting_identity.as_ref().is_some_and(|starting| identity_matches(starting, &identity))
        });
        // alive な self は別 identity で置換しない。退役済み self を非 Removed の別 identity で
        // 巻き戻すのは再参加遷移か開始対象 identity に限り、古い incarnation の遅延 Up では発火させない。
        let retired_blocks_non_removed =
          current_is_retired && *to != NodeStatus::Removed && !can_rejoin && !incoming_is_starting_identity;
        if !current_is_removed || retired_blocks_non_removed {
          return;
        }
      }
      if let Some(current) = self.lifecycle.self_identity.with_lock(|self_identity| self_identity.clone())
        && identity_matches(&current, &identity)
        && current_status.as_ref().is_some_and(|status| status.status == NodeStatus::Removed)
        && *to != NodeStatus::Removed
        && !can_rejoin
      {
        return;
      }
      if !trigger_member_status_callback::<F>(&self.callback_state, node_id, authority) {
        return;
      }
      let subscription_id = self.state.with_lock(|state| {
        state.unsubscribe_requested = true;
        state.subscription_id
      });
      if let Some(id) = subscription_id {
        self.event_stream.unsubscribe(id);
      }
    }
  }
}

struct NoopMemberStatusSubscriber;

impl EventStreamSubscriber for NoopMemberStatusSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {}
}

/// Cluster extension registered into `ActorSystem`.
pub struct ClusterExtension {
  core: SharedLock<ClusterCore>,
  event_stream: EventStreamShared,
  grain_metrics: Option<GrainMetricsShared>,
  subscription: SharedLock<Option<EventStreamSubscription>>,
  terminated: SharedLock<bool>,
  self_member_status: SharedLock<Option<SelfMemberStatus>>,
  self_member_identity: SharedLock<Option<SelfMemberIdentity>>,
  suppressed_retired_identities: SharedLock<Vec<SelfMemberIdentity>>,
  starting_identity: SharedLock<Option<SelfMemberIdentity>>,
  start_in_progress: SharedLock<bool>,
  topology_absent_identities: SharedLock<Vec<SelfMemberIdentity>>,
  idle_passivation_task: SharedLock<Option<SchedulerHandle>>,
  idle_passivation_actor: SharedLock<Option<ActorRef>>,
  _self_member_status_subscription: EventStreamSubscription,
  _system: ActorSystemWeak,
}

impl ClusterExtension {
  /// Creates the extension from injected dependencies.
  ///
  /// Uses a weak reference to the actor system to avoid circular references.
  #[must_use]
  pub fn new(system: &ActorSystem, core: ClusterCore) -> Self {
    let event_stream = system.event_stream();
    let self_address = core.startup_address();
    let grain_metrics = if core.metrics_enabled() { Some(GrainMetricsShared::new(GrainMetrics::new())) } else { None };
    let self_member_status = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    let self_member_identity = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    let suppressed_retired_identities = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
    let starting_identity = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    let start_in_progress = SharedLock::new_with_driver::<DefaultMutex<_>>(false);
    let topology_absent_identities = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
    let terminated = SharedLock::new_with_driver::<DefaultMutex<_>>(false);
    let idle_passivation_task = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    let idle_passivation_actor = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    let status_subscriber = subscriber_handle(SelfMemberStatusTrackerSubscriber::new(
      self_address,
      self_member_status.clone(),
      self_member_identity.clone(),
      terminated.clone(),
      suppressed_retired_identities.clone(),
      starting_identity.clone(),
      start_in_progress.clone(),
      topology_absent_identities.clone(),
    ));
    let self_member_status_subscription = event_stream.subscribe_no_replay(&status_subscriber);
    let locked = SharedLock::new_with_driver::<DefaultMutex<_>>(core);
    let subscription = SharedLock::new_with_driver::<DefaultMutex<_>>(None);
    Self {
      core: locked,
      event_stream,
      grain_metrics,
      subscription,
      terminated,
      self_member_status,
      self_member_identity,
      suppressed_retired_identities,
      starting_identity,
      start_in_progress,
      topology_absent_identities,
      idle_passivation_task,
      idle_passivation_actor,
      _self_member_status_subscription: self_member_status_subscription,
      _system: system.downgrade(),
    }
  }

  /// Returns the shared cluster core handle.
  #[must_use]
  pub(crate) fn core_shared(&self) -> SharedLock<ClusterCore> {
    self.core.clone()
  }

  /// Returns the shared pub/sub handle.
  #[must_use]
  pub(crate) fn pub_sub_shared(&self) -> ClusterPubSubShared {
    self.core.with_lock(|core| core.pub_sub_shared())
  }

  /// Returns the shared grain metrics handle if enabled.
  #[must_use]
  pub(crate) fn grain_metrics_shared(&self) -> Option<GrainMetricsShared> {
    self.grain_metrics.clone()
  }

  pub(crate) fn publish_activation_events(&self, events: Vec<PlacementEvent>) {
    publish_activation_events(&self.event_stream, &self.grain_metrics, events);
  }

  fn prepare_idle_passivation_task(&self) -> Result<bool, ClusterError> {
    self.core.with_lock(|core| core.validate_configuration()).map_err(ClusterError::from)?;
    let interval = self.core.with_lock(|core| core.grain_idle_passivation_threshold());
    if self.idle_passivation_task.with_lock(|task| task.is_some()) {
      return Ok(false);
    }
    let system = self._system.upgrade().ok_or_else(|| ClusterError::GrainIdlePassivationScheduler {
      reason: String::from("actor system is unavailable"),
    })?;
    let scheduler = system.state().scheduler();
    let sweep_interval = interval.min(scheduler.maximum_delay());
    let actor_ref = self.idle_passivation_actor.with_lock(|actor| -> Result<ActorRef, ClusterError> {
      if let Some(actor_ref) = actor.clone() {
        return Ok(actor_ref);
      }
      let core = self.core.clone();
      let event_stream = self.event_stream.clone();
      let grain_metrics = self.grain_metrics.clone();
      let props = Props::from_fn(move || {
        GrainIdlePassivationActor::new(core.clone(), event_stream.clone(), grain_metrics.clone())
      });
      let actor_ref = system
        .extended()
        .spawn_system_actor(&props)
        .map_err(|error| ClusterError::GrainIdlePassivationScheduler {
          reason: format!("idle passivation actor spawn failed: {error}"),
        })?
        .into_actor_ref();
      *actor = Some(actor_ref.clone());
      Ok(actor_ref)
    })?;
    if self.idle_passivation_task.with_lock(|task| task.is_some()) {
      return Ok(false);
    }
    let resolution = scheduler.with_read(|scheduler| scheduler.resolution());
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(move |batch: &ExecutionBatch| {
      let nanos = resolution.as_nanos().saturating_mul(u128::from(batch.execution_tick()));
      let now = u64::try_from(nanos).unwrap_or(u64::MAX);
      let mut receiver = actor_ref.clone();
      receiver.try_tell(AnyMessage::new(now)).unwrap_or_else(|error| report_idle_passivation_delivery_failure(&error));
    });
    let command = SchedulerCommand::RunRunnable { runnable };
    let handle = scheduler
      .with_write(|scheduler| {
        scheduler.schedule_with_fixed_delay_skipping_missed(sweep_interval, sweep_interval, command)
      })
      .map_err(|error| ClusterError::GrainIdlePassivationScheduler { reason: format!("{error}") })?;
    let duplicate = self.idle_passivation_task.with_lock(|task| {
      if task.is_some() {
        return true;
      }
      *task = Some(handle.clone());
      false
    });
    if duplicate && !scheduler.with_write(|scheduler| scheduler.cancel(&handle)) {
      return Err(ClusterError::GrainIdlePassivationScheduler {
        reason: String::from("duplicate idle passivation task cancellation failed"),
      });
    }
    Ok(!duplicate)
  }

  fn cancel_idle_passivation_task(&self) -> Result<(), ClusterError> {
    let Some(handle) = self.idle_passivation_task.with_lock(|task| task.take()) else {
      return Ok(());
    };
    let cancelled = self._system.upgrade().map_or_else(
      || handle.cancel() || handle.is_cancelled() || handle.is_completed(),
      |system| {
        system.state().scheduler().with_write(|scheduler| scheduler.cancel(&handle))
          || handle.is_cancelled()
          || handle.is_completed()
      },
    );
    if cancelled {
      return Ok(());
    }
    self.idle_passivation_task.with_lock(|task| *task = Some(handle));
    Err(ClusterError::GrainIdlePassivationScheduler {
      reason: String::from("idle passivation task cancellation failed"),
    })
  }

  fn publish_pre_start_failure(&self, mode: StartupMode, error: &ClusterError) {
    let reason = match error {
      | ClusterError::Configuration(error) => error.to_string(),
      | ClusterError::GrainIdlePassivationScheduler { reason } => reason.clone(),
      | _ => format!("{error:?}"),
    };
    self.core.with_lock(|core| core.publish_startup_failure(mode, reason));
  }

  /// Subscribes to the event stream for topology updates.
  fn subscribe_topology_events(&self) {
    // 既に購読中なら何もしない
    if self.subscription.with_lock(|subscription| subscription.is_some()) {
      return;
    }

    // ClusterCore への共有参照を持つ subscriber を作成
    let self_address = self.core.with_lock(|core| core.startup_address());
    let subscriber: ClusterTopologySubscriber = ClusterTopologySubscriber::new(
      self.core.clone(),
      self.event_stream.clone(),
      self_address,
      self.self_member_status.clone(),
      self.self_member_identity.clone(),
      self.starting_identity.clone(),
      self.topology_absent_identities.clone(),
    );
    let subscriber_handle = subscriber_handle(subscriber);
    let sub = self.event_stream.subscribe(&subscriber_handle);
    self.subscription.with_lock(|subscription| *subscription = Some(sub));
  }

  /// Starts member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&self) -> Result<(), ClusterError> {
    let previous_status = self.self_member_status.with_lock(|self_member_status| self_member_status.clone());
    let previous_identity = self.self_member_identity.with_lock(|self_member_identity| self_member_identity.clone());
    self.self_member_status.with_lock(|self_member_status| *self_member_status = None);
    self.self_member_identity.with_lock(|self_member_identity| *self_member_identity = None);
    self.starting_identity.with_lock(|starting_identity| *starting_identity = previous_identity.clone());
    if let Some(previous_identity) = previous_identity.clone() {
      remember_retired_identity(&self.suppressed_retired_identities, previous_identity);
    }
    let was_terminated = self.terminated.with_lock(|terminated| {
      let was_terminated = *terminated;
      *terminated = false;
      was_terminated
    });
    self.start_in_progress.with_lock(|start_in_progress| *start_in_progress = true);
    let (result, passivation_task_created) = match self.prepare_idle_passivation_task() {
      | Ok(created) => (self.core.with_lock(|core| core.start_member()), created),
      | Err(error) => {
        self.publish_pre_start_failure(StartupMode::Member, &error);
        (Err(error), false)
      },
    };
    self.start_in_progress.with_lock(|start_in_progress| *start_in_progress = false);
    if result.is_ok() {
      self.subscribe_topology_events();
    } else {
      let cancel_result = if passivation_task_created { self.cancel_idle_passivation_task() } else { Ok(()) };
      let failed_identity = self.self_member_identity.with_lock(|self_member_identity| self_member_identity.clone());
      if let Some(failed_identity) = failed_identity
        && previous_identity.as_ref().is_none_or(|previous| !identity_matches(previous, &failed_identity))
      {
        remember_retired_identity(&self.suppressed_retired_identities, failed_identity);
      }
      self.self_member_status.with_lock(|self_member_status| *self_member_status = previous_status);
      self.self_member_identity.with_lock(|self_member_identity| *self_member_identity = previous_identity.clone());
      if let Some(previous_identity) = previous_identity {
        retain_non_matching_identity(&self.suppressed_retired_identities, &previous_identity);
      }
      if was_terminated {
        self.terminated.with_lock(|terminated| *terminated = true);
      }
      self.starting_identity.with_lock(|starting_identity| *starting_identity = None);
      cancel_result?;
    }
    result
  }

  /// Starts client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if pub/sub or provider startup fails.
  pub fn start_client(&self) -> Result<(), ClusterError> {
    let previous_status = self.self_member_status.with_lock(|self_member_status| self_member_status.clone());
    let previous_identity = self.self_member_identity.with_lock(|self_member_identity| self_member_identity.clone());
    self.self_member_status.with_lock(|self_member_status| *self_member_status = None);
    self.self_member_identity.with_lock(|self_member_identity| *self_member_identity = None);
    self.starting_identity.with_lock(|starting_identity| *starting_identity = previous_identity.clone());
    if let Some(previous_identity) = previous_identity.clone() {
      remember_retired_identity(&self.suppressed_retired_identities, previous_identity);
    }
    let was_terminated = self.terminated.with_lock(|terminated| {
      let was_terminated = *terminated;
      *terminated = false;
      was_terminated
    });
    self.start_in_progress.with_lock(|start_in_progress| *start_in_progress = true);
    let (result, passivation_task_created) = match self.prepare_idle_passivation_task() {
      | Ok(created) => (self.core.with_lock(|core| core.start_client()), created),
      | Err(error) => {
        self.publish_pre_start_failure(StartupMode::Client, &error);
        (Err(error), false)
      },
    };
    self.start_in_progress.with_lock(|start_in_progress| *start_in_progress = false);
    if result.is_ok() {
      self.subscribe_topology_events();
    } else {
      let cancel_result = if passivation_task_created { self.cancel_idle_passivation_task() } else { Ok(()) };
      let failed_identity = self.self_member_identity.with_lock(|self_member_identity| self_member_identity.clone());
      if let Some(failed_identity) = failed_identity
        && previous_identity.as_ref().is_none_or(|previous| !identity_matches(previous, &failed_identity))
      {
        remember_retired_identity(&self.suppressed_retired_identities, failed_identity);
      }
      self.self_member_status.with_lock(|self_member_status| *self_member_status = previous_status);
      self.self_member_identity.with_lock(|self_member_identity| *self_member_identity = previous_identity.clone());
      if let Some(previous_identity) = previous_identity {
        retain_non_matching_identity(&self.suppressed_retired_identities, &previous_identity);
      }
      if was_terminated {
        self.terminated.with_lock(|terminated| *terminated = true);
      }
      self.starting_identity.with_lock(|starting_identity| *starting_identity = None);
      cancel_result?;
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
    self.subscription.with_lock(|subscription| *subscription = None);
    let result = self.core.with_lock(|core| core.shutdown(graceful));
    if result.is_ok() {
      self.cancel_idle_passivation_task()?;
      self.terminated.with_lock(|terminated| *terminated = true);
      self.self_member_status.with_lock(|self_member_status| *self_member_status = None);
    }
    result
  }

  /// Explicitly downs the provided member authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or the provider cannot process downing.
  pub fn down(&self, authority: &str) -> Result<(), ClusterError> {
    self.core.with_lock(|core| core.down(authority))
  }

  /// Requests a member join for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or join processing fails.
  pub fn join(&self, authority: &str) -> Result<(), ClusterError> {
    self.core.with_lock(|core| core.join(authority))
  }

  /// Requests a graceful member leave for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or leave processing fails.
  pub fn leave(&self, authority: &str) -> Result<(), ClusterError> {
    self.core.with_lock(|core| core.leave(authority))
  }

  /// Starts full-cluster shutdown preparation.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started.
  pub fn prepare_for_full_cluster_shutdown(&self) -> Result<(), ClusterError> {
    let events = self.core.with_lock(|core| core.prepare_for_full_cluster_shutdown())?;
    for event in events {
      self.publish_cluster_event(event);
    }
    Ok(())
  }

  /// Registers kinds for member mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_member_kinds(&self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.core.with_lock(|core| core.setup_member_kinds(kinds))
  }

  /// Registers kinds for client mode.
  ///
  /// # Errors
  ///
  /// Returns an error if identity lookup setup fails.
  pub fn setup_client_kinds(&self, kinds: Vec<ActivatedKind>) -> Result<(), IdentitySetupError> {
    self.core.with_lock(|core| core.setup_client_kinds(kinds))
  }

  /// Applies topology updates.
  ///
  /// This method applies the topology and publishes the event to EventStream.
  /// The lock is released before publishing to avoid deadlocks with subscribers.
  pub fn on_topology(&self, update: &TopologyUpdate) {
    // ロックを保持したまま publish するとデッドロックするため、
    // イベントを取得してからロックを解放し、その後に publish する
    let result = self.core.with_lock(|core| core.try_apply_topology(update));
    if matches!(result.as_ref(), Ok(Some(_))) {
      let self_address = self.core.with_lock(|core| core.startup_address());
      clear_self_observation_if_absent(
        update,
        &self_address,
        &self.self_member_status,
        &self.self_member_identity,
        &self.starting_identity,
        &self.topology_absent_identities,
      );
    }

    match result {
      | Ok(Some(event)) => {
        let payload = AnyMessage::new(event);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        self.event_stream.publish(&extension_event);
      },
      | Ok(None) => {},
      | Err(error) => {
        let reason = format!("{error:?}");
        let failed = ClusterEvent::TopologyApplyFailed { reason, observed_at: update.observed_at };
        let payload = AnyMessage::new(failed);
        let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
        self.event_stream.publish(&extension_event);
      },
    }
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessage::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from(CLUSTER_EVENT_STREAM_NAME), payload };
    self.event_stream.publish(&extension_event);
  }

  /// Registers a callback invoked when a member reaches `Up` status.
  #[must_use]
  pub fn register_on_member_up<F>(&self, callback: F) -> EventStreamSubscription
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    self.register_on_member_status(NodeStatus::Up, callback)
  }

  /// Registers a callback invoked when a member reaches `Removed` status.
  #[must_use]
  pub fn register_on_member_removed<F>(&self, callback: F) -> EventStreamSubscription
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    if self.terminated.with_lock(|terminated| *terminated) {
      let self_address = self.core.with_lock(|core| core.startup_address());
      let node_id = {
        let current = self.self_member_identity.with_lock(|self_member_identity| self_member_identity.clone());
        if let Some(identity) = current.as_ref() {
          debug_assert_eq!(identity.authority, self_address);
          identity.node_id.clone()
        } else {
          self_address.clone()
        }
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
    self.core.with_lock(|core| core.metrics())
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
  #[must_use]
  pub fn virtual_actor_count(&self) -> i64 {
    self.core.with_lock(|core| core.virtual_actor_count())
  }

  /// Builds a snapshot of the inputs for grain readiness derivation.
  ///
  /// The snapshot reflects the runtime state at the time of the call.
  /// Continuous monitoring is the caller's responsibility. Probe endpoint
  /// wiring (HTTP servers etc.) is out of scope and owned by the caller.
  #[must_use]
  pub fn grain_readiness_snapshot(&self) -> GrainReadinessSnapshot {
    let observed_self_status =
      self.self_member_status.with_lock(|self_status| self_status.as_ref().map(|status| status.status));
    self.core.with_lock(|core| {
      if core.mode().is_some() && core.has_current_self_member() && observed_self_status.is_some() {
        return core.grain_readiness_snapshot_with_self_status(observed_self_status);
      }
      core.grain_readiness_snapshot()
    })
  }

  /// Returns blocked members cache.
  #[must_use]
  pub fn blocked_members(&self) -> Vec<String> {
    self.core.with_lock(|core| core.blocked_members().to_vec())
  }

  fn register_on_member_status<F>(&self, target: NodeStatus, callback: F) -> EventStreamSubscription
  where
    F: FnMut(&str, &str) + Send + Sync + 'static, {
    let self_address = self.core.with_lock(|core| core.startup_address());
    let state = SharedLock::new_with_driver::<DefaultMutex<_>>(MemberStatusSubscriberState::new());
    let callback_state = SharedLock::new_with_driver::<DefaultMutex<_>>(MemberStatusCallbackState::new(callback));
    let lifecycle = SelfMemberLifecycle::new(
      self.self_member_status.clone(),
      self.self_member_identity.clone(),
      self.terminated.clone(),
      self.suppressed_retired_identities.clone(),
      self.starting_identity.clone(),
      self.start_in_progress.clone(),
      self.topology_absent_identities.clone(),
    );
    let subscriber = subscriber_handle(MemberStatusSubscriber::new(
      target,
      self_address.clone(),
      callback_state.clone(),
      state.clone(),
      self.event_stream.clone(),
      lifecycle,
    ));
    let subscription = self.event_stream.subscribe_no_replay(&subscriber);
    let subscription_id = subscription.id();
    state.with_lock(|guard| {
      guard.subscription_id = Some(subscription_id);
    });
    if let Some(current) = self.self_member_status.with_lock(|self_member_status| self_member_status.clone())
      && current.authority == self_address.as_str()
      && current.status == target
      && !is_suppressed_retired_identity(&self.suppressed_retired_identities, &SelfMemberIdentity {
        node_id:   current.node_id.clone(),
        authority: current.authority.clone(),
      })
      && trigger_member_status_callback::<F>(&callback_state, &current.node_id, &current.authority)
    {
      state.with_lock(|state| state.unsubscribe_requested = true);
    }
    let unsubscribe_now = state.with_lock(|state| state.unsubscribe_requested);
    if unsubscribe_now {
      self.event_stream.unsubscribe(subscription_id);
    }
    subscription
  }

  fn already_unsubscribed_subscription(&self) -> EventStreamSubscription {
    let subscriber = subscriber_handle(NoopMemberStatusSubscriber);
    let subscription = self.event_stream.subscribe(&subscriber);
    self.event_stream.unsubscribe(subscription.id());
    subscription
  }
}

pub(super) fn publish_activation_events(
  event_stream: &EventStreamShared,
  metrics: &Option<GrainMetricsShared>,
  events: Vec<PlacementEvent>,
) {
  for event in events {
    match event {
      | PlacementEvent::Activated { key, pid, .. } => {
        publish_grain_event(event_stream, GrainEvent::ActivationCreated { key, pid });
        if let Some(metrics) = metrics {
          metrics.with_write(|inner| inner.record_activation_created());
        }
      },
      | PlacementEvent::Passivated { key, .. } => {
        publish_grain_event(event_stream, GrainEvent::ActivationPassivated { key });
        if let Some(metrics) = metrics {
          metrics.with_write(|inner| inner.record_activation_passivated());
        }
      },
      | _ => {},
    }
  }
}

fn publish_grain_event(event_stream: &EventStreamShared, event: GrainEvent) {
  let payload = AnyMessage::new(event);
  let extension_event = EventStreamEvent::Extension { name: String::from(GRAIN_EVENT_STREAM_NAME), payload };
  event_stream.publish(&extension_event);
}

impl fraktor_actor_core_kernel_rs::actor::extension::Extension for ClusterExtension {}
