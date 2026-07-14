//! Core cluster state holder wiring dependencies and configuration.

#[cfg(test)]
#[path = "cluster_core_test.rs"]
mod tests;

use alloc::{
  boxed::Box,
  collections::{BTreeMap, BTreeSet},
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess, SharedLock},
  time::TimerInstant,
};

use crate::{
  BlockListProvider, ClusterError, ClusterEvent, ClusterExtensionConfig, ClusterExtensionConfigError, ClusterMetrics,
  ClusterMetricsSnapshot, ClusterProviderError, ClusterProviderShared, MetricsError, StartupMode, TopologyApplyError,
  TopologyUpdate,
  activation::{
    ActivatedKind, IdentityLookupShared, IdentitySetupError, LookupError, PidCache, PlacementEvent, PlacementResolution,
  },
  downing_provider::{DowningDecision, DowningInput, DowningProvider},
  failure_detector::FailureDetectorConfig,
  grain::{GrainKey, GrainReadinessSnapshot, KindRegistry},
  membership::{CurrentClusterState, GossiperShared, MembershipVersion, NodeRecord, NodeStatus},
  pub_sub::ClusterPubSubShared,
};

/// Aggregates configuration and shared dependencies for cluster runtime flows.
pub struct ClusterCore {
  provider: ClusterProviderShared,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  event_stream: EventStreamShared,
  downing_provider: SharedLock<Box<dyn DowningProvider>>,
  gossiper: GossiperShared,
  pub_sub: ClusterPubSubShared,
  failure_detector_config: FailureDetectorConfig,
  grain_idle_passivation_threshold: Duration,
  startup_state: ClusterStartupState,
  metrics_enabled: bool,
  kind_registry: KindRegistry,
  identity_lookup: IdentityLookupShared,
  virtual_actor_count: i64,
  mode: Option<StartupMode>,
  metrics: Option<ClusterMetrics>,
  blocked_members: Vec<String>,
  member_count: usize,
  pid_cache: Option<PidCache>,
  last_topology_hash: Option<u64>,
  current_members: Vec<String>,
  observed_at: TimerInstant,
  preparing_for_shutdown: bool,
  shutdown_prepared_members: BTreeSet<String>,
}

impl ClusterCore {
  pub(crate) fn validate_configuration(&self) -> Result<(), ClusterExtensionConfigError> {
    self.failure_detector_config.validate()?;
    ClusterExtensionConfig::validate_grain_idle_passivation_threshold(self.grain_idle_passivation_threshold)
  }

  /// Builds a new cluster core from the provided dependencies.
  #[must_use]
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    config: &ClusterExtensionConfig,
    provider: ClusterProviderShared,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    event_stream: EventStreamShared,
    downing_provider: SharedLock<Box<dyn DowningProvider>>,
    gossiper: GossiperShared,
    pubsub: ClusterPubSubShared,
    kind_registry: KindRegistry,
    identity_lookup: IdentityLookupShared,
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
      downing_provider,
      gossiper,
      pub_sub: pubsub,
      failure_detector_config: *config.failure_detector_config(),
      grain_idle_passivation_threshold: config.grain_idle_passivation_threshold(),
      startup_state,
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
      current_members: Vec::new(),
      observed_at: TimerInstant::zero(Duration::from_secs(1)),
      preparing_for_shutdown: false,
      shutdown_prepared_members: BTreeSet::new(),
    }
  }

  /// Returns whether metrics collection is enabled.
  #[must_use]
  pub const fn metrics_enabled(&self) -> bool {
    self.metrics_enabled
  }

  /// Returns the configured Grain idle passivation threshold.
  #[must_use]
  pub(crate) const fn grain_idle_passivation_threshold(&self) -> Duration {
    self.grain_idle_passivation_threshold
  }

  /// Returns the advertised address shared by member and client modes.
  #[must_use]
  pub(crate) fn startup_address(&self) -> String {
    self.startup_state.address.clone()
  }

  /// Returns the current startup mode when the cluster is running.
  #[must_use]
  pub const fn mode(&self) -> Option<StartupMode> {
    self.mode
  }

  /// Returns true when the current topology still contains the self authority.
  #[must_use]
  pub(crate) fn has_current_self_member(&self) -> bool {
    self.current_members.iter().any(|authority| authority == &self.startup_state.address)
  }

  /// Returns true if the given kind is registered.
  #[must_use]
  pub fn is_kind_registered(&self, kind: &str) -> bool {
    self.kind_registry.contains(kind)
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
    self.identity_lookup.with_write(|lookup| lookup.setup_member(&snapshot))?;
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
    self.identity_lookup.with_write(|lookup| lookup.setup_client(&snapshot))?;
    Ok(())
  }

  /// Resolves a PID for the given grain key.
  ///
  /// # Errors
  ///
  /// Returns an error when placement resolution fails or is pending.
  pub fn resolve_pid(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    self.identity_lookup.with_write(|lookup| lookup.resolve(key, now))
  }

  /// Resolves a PID while preserving exact monotonic time for idle tracking.
  ///
  /// # Errors
  ///
  /// Returns an error when placement resolution fails or is pending.
  pub(crate) fn resolve_pid_at(
    &mut self,
    key: &GrainKey,
    now_secs: u64,
    idle_now_nanos: u64,
  ) -> Result<PlacementResolution, LookupError> {
    self.identity_lookup.with_write(|lookup| lookup.resolve_at(key, now_secs, idle_now_nanos))
  }

  /// Passivates activations that exceeded the configured idle threshold.
  pub(crate) fn passivate_idle_at(&mut self, now_nanos: u64) {
    let idle_ttl_nanos = u64::try_from(self.grain_idle_passivation_threshold.as_nanos()).unwrap_or(u64::MAX);
    self.identity_lookup.with_write(|lookup| lookup.passivate_idle_at(now_nanos, idle_ttl_nanos));
  }

  /// Drains placement events emitted by identity lookup.
  #[must_use]
  pub(crate) fn drain_placement_events(&mut self) -> Vec<PlacementEvent> {
    self.identity_lookup.with_write(|lookup| lookup.drain_events())
  }

  /// Returns the aggregated virtual actor count.
  #[must_use]
  pub const fn virtual_actor_count(&self) -> i64 {
    self.virtual_actor_count
  }

  /// Returns the shared pub/sub handle.
  #[must_use]
  pub(crate) fn pub_sub_shared(&self) -> ClusterPubSubShared {
    self.pub_sub.clone()
  }

  /// Returns the cached blocked members retrieved from the provider.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn blocked_members(&self) -> &[String] {
    &self.blocked_members
  }

  /// Returns the current cluster-state snapshot and its observation time.
  #[must_use]
  pub fn current_cluster_state_snapshot(&self) -> (CurrentClusterState, TimerInstant) {
    let version = MembershipVersion::new(self.last_topology_hash.unwrap_or(0));
    let status = if self.preparing_for_shutdown { NodeStatus::PreparingForShutdown } else { NodeStatus::Up };
    let members = self
      .current_members
      .iter()
      .cloned()
      .map(|authority| {
        let node_id = authority.clone();
        NodeRecord::new(node_id, authority, status, version, String::new(), Vec::new())
      })
      .collect::<Vec<_>>();
    let leader = members.iter().map(|record| record.authority.clone()).min();
    let state = CurrentClusterState::new(members, Vec::new(), Vec::new(), leader, BTreeMap::new());
    (state, self.observed_at)
  }

  /// Builds a snapshot of the inputs for grain readiness derivation.
  ///
  /// Reads only existing state: the self node status taken from the current
  /// cluster-state snapshot (absent when the self node is not a member), the
  /// placement coordination state observed through the identity lookup port,
  /// and the registered kind names. It does not mutate any runtime state.
  #[must_use]
  pub fn grain_readiness_snapshot(&self) -> GrainReadinessSnapshot {
    let (state, _) = self.current_cluster_state_snapshot();
    let self_address = self.startup_address();
    let self_status = state.members.iter().find(|record| record.authority == self_address).map(|record| record.status);
    self.grain_readiness_snapshot_with_self_status(self_status)
  }

  /// Builds a readiness snapshot using the provided observed self-node status.
  #[must_use]
  pub(crate) fn grain_readiness_snapshot_with_self_status(
    &self,
    self_status: Option<NodeStatus>,
  ) -> GrainReadinessSnapshot {
    let placement_state = self.identity_lookup.with_read(|lookup| lookup.placement_state());
    let registered_kinds = self.kind_registry.all().into_iter().map(|kind| kind.name().to_string()).collect();
    GrainReadinessSnapshot::new(self_status, placement_state, registered_kinds)
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
  /// Returns an error if configuration validation, pub/sub, gossiper, or provider startup fails.
  pub fn start_member(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    self.validate_configuration().map_err(|error| {
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode:    StartupMode::Member,
        reason:  error.to_string(),
      });
      ClusterError::from(error)
    })?;
    self.refresh_blocked_members();

    self.pub_sub.with_write(|pub_sub| pub_sub.start()).map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode: StartupMode::Member,
        reason,
      });
      error
    })?;

    self.gossiper.with_write(|gossiper| gossiper.start()).map_err(|reason| {
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode:    StartupMode::Member,
        reason:  reason.to_string(),
      });
      ClusterError::Gossip(reason)
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.with_write(|provider| provider.start_member());
    match provider_result {
      | Ok(()) => {
        self.mode = Some(StartupMode::Member);
        self.last_topology_hash = None;
        self.member_count = 1;
        self.current_members = Vec::from([address.clone()]);
        self.observed_at = TimerInstant::zero(Duration::from_secs(1));
        self.preparing_for_shutdown = false;
        self.shutdown_prepared_members.clear();
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
  /// Returns an error if configuration validation, pub/sub, gossiper, or provider startup fails.
  pub fn start_client(&mut self) -> Result<(), ClusterError> {
    let address = self.startup_address();
    self.validate_configuration().map_err(|error| {
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode:    StartupMode::Client,
        reason:  error.to_string(),
      });
      ClusterError::from(error)
    })?;
    self.refresh_blocked_members();

    self.pub_sub.with_write(|pub_sub| pub_sub.start()).map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode: StartupMode::Client,
        reason,
      });
      error
    })?;

    self.gossiper.with_write(|gossiper| gossiper.start()).map_err(|reason| {
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: address.clone(),
        mode:    StartupMode::Client,
        reason:  reason.to_string(),
      });
      ClusterError::Gossip(reason)
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.with_write(|provider| provider.start_client());
    match provider_result {
      | Ok(()) => {
        self.mode = Some(StartupMode::Client);
        self.last_topology_hash = None;
        self.member_count = 1;
        self.current_members = Vec::from([address.clone()]);
        self.observed_at = TimerInstant::zero(Duration::from_secs(1));
        self.preparing_for_shutdown = false;
        self.shutdown_prepared_members.clear();
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
    self.pub_sub.with_write(|pub_sub| pub_sub.stop()).map_err(ClusterError::from).map_err(|error| {
      let reason = format!("pubsub: {error:?}");
      self.publish_cluster_event(ClusterEvent::ShutdownFailed { address: address.clone(), mode, reason });
      error
    })?;

    self.gossiper.with_write(|gossiper| gossiper.stop()).map_err(|reason| {
      self.publish_cluster_event(ClusterEvent::ShutdownFailed {
        address: address.clone(),
        mode,
        reason: reason.to_string(),
      });
      ClusterError::Gossip(reason)
    })?;

    // ガードを早期ドロップさせるため、結果を先に取得
    let provider_result = self.provider.with_write(|provider| provider.shutdown(graceful));
    match provider_result {
      | Ok(()) => {
        self.virtual_actor_count = 0;
        self.member_count = 0;
        self.update_metrics(self.member_count, 0);
        self.blocked_members.clear();
        self.current_members.clear();
        self.observed_at = TimerInstant::zero(Duration::from_secs(1));
        self.last_topology_hash = None;
        self.preparing_for_shutdown = false;
        self.shutdown_prepared_members.clear();
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

  /// Explicitly downs a member authority.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster has not been started, strategy evaluation fails, the strategy
  /// does not return [`DowningDecision::Down`] for the explicit command, or provider-side down
  /// processing fails after a [`DowningDecision::Down`] decision.
  pub fn down(&mut self, authority: &str) -> Result<(), ClusterError> {
    if self.mode.is_none() {
      return Err(ClusterError::from(ClusterProviderError::down("cluster is not started")));
    }
    let input = DowningInput::explicit_down(authority);
    let decision = self
      .downing_provider
      .with_lock(|downing_provider| downing_provider.decide(&input))
      .map_err(ClusterError::from)?;
    match decision {
      | DowningDecision::Down => {
        self.provider.with_write(|provider| provider.down(authority)).map_err(ClusterError::from)
      },
      | DowningDecision::Keep | DowningDecision::Defer => {
        Err(ClusterError::DowningRejected { authority: String::from(authority), decision })
      },
    }
  }

  /// Requests a member join for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster is not started or the provider rejects the join request.
  pub fn join(&mut self, authority: &str) -> Result<(), ClusterError> {
    if self.mode.is_none() {
      return Err(ClusterError::from(ClusterProviderError::join("cluster is not started")));
    }
    self.provider.with_write(|provider| provider.join(authority)).map_err(ClusterError::from)
  }

  /// Requests a graceful member leave for the provided authority.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster is not started or the provider rejects the leave request.
  pub fn leave(&mut self, authority: &str) -> Result<(), ClusterError> {
    if self.mode.is_none() {
      return Err(ClusterError::from(ClusterProviderError::leave("cluster is not started")));
    }
    self.provider.with_write(|provider| provider.leave(authority)).map_err(ClusterError::from)
  }

  /// Starts full-cluster shutdown preparation.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster has not been started.
  pub fn prepare_for_full_cluster_shutdown(&mut self) -> Result<Vec<ClusterEvent>, ClusterError> {
    if self.mode != Some(StartupMode::Member) {
      return Err(ClusterError::from(ClusterProviderError::shutdown(
        "full-cluster shutdown preparation requires member mode",
      )));
    }
    self.preparing_for_shutdown = true;
    let observed_at = self.observed_at;
    let members_to_prepare = self
      .current_members
      .iter()
      .filter(|authority| !self.shutdown_prepared_members.contains(*authority))
      .cloned()
      .collect::<Vec<_>>();
    if members_to_prepare.is_empty() {
      return Ok(Vec::new());
    }

    let mut events = Vec::new();
    let (prepared_state, _) = self.current_cluster_state_snapshot();
    events.push(ClusterEvent::CurrentClusterState { state: prepared_state, observed_at });
    for authority in members_to_prepare {
      self.shutdown_prepared_members.insert(authority.clone());
      events.push(ClusterEvent::MemberStatusChanged {
        node_id: authority.clone(),
        authority: authority.clone(),
        from: NodeStatus::Up,
        to: NodeStatus::PreparingForShutdown,
        observed_at,
      });
      events.push(ClusterEvent::MemberPreparingForShutdown { node_id: authority.clone(), authority, observed_at });
    }
    Ok(events)
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessage::new(event);
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
  ///
  /// # Errors
  ///
  /// Returns [`TopologyApplyError::NotStarted`] if the cluster is not running,
  /// or [`TopologyApplyError::InvalidTopology`] when the update is invalid.
  pub fn try_apply_topology(&mut self, update: &TopologyUpdate) -> Result<Option<ClusterEvent>, TopologyApplyError> {
    if self.mode.is_none() {
      return Err(TopologyApplyError::NotStarted);
    }

    validate_topology_update(update)?;

    if self.apply_topology_internal(update) {
      Ok(Some(ClusterEvent::TopologyUpdated { update: update.clone() }))
    } else {
      Ok(None)
    }
  }

  /// Applies a topology update without publishing an event.
  ///
  /// Use this method when the topology update was already received via EventStream
  /// to avoid re-publishing and causing infinite loops.
  ///
  /// # Errors
  ///
  /// Returns [`TopologyApplyError`] if the update cannot be applied.
  pub fn apply_topology(&mut self, update: &TopologyUpdate) -> Result<(), TopologyApplyError> {
    let _ = self.try_apply_topology(update)?;
    Ok(())
  }

  /// Internal helper that applies topology and returns whether the update was applied.
  fn apply_topology_internal(&mut self, update: &TopologyUpdate) -> bool {
    if self.last_topology_hash == Some(update.topology.hash()) {
      return false;
    }
    self.last_topology_hash = Some(update.topology.hash());
    self.blocked_members = update.blocked.clone();

    self.member_count = update.members.len();
    self.update_metrics(self.member_count, self.virtual_actor_count);
    self.current_members = update.members.clone();
    self.observed_at = update.observed_at;
    if self.preparing_for_shutdown {
      let members = &self.current_members;
      self.shutdown_prepared_members.retain(|authority| members.contains(authority));
    }

    if let Some(cache) = self.pid_cache.as_mut() {
      for authority in update.left.iter().chain(update.dead.iter()) {
        cache.invalidate_authority(authority);
      }
    }

    let members = update.members.clone();
    let left = update.left.clone();
    let dead = update.dead.clone();
    self.identity_lookup.with_write(|identity_lookup| {
      identity_lookup.update_topology(members);
      for authority in left.iter().chain(dead.iter()) {
        identity_lookup.on_member_left(authority);
      }
    });

    self.pub_sub.with_write(|pub_sub| pub_sub.on_topology(update));

    true
  }
}

fn validate_topology_update(update: &TopologyUpdate) -> Result<(), TopologyApplyError> {
  let joined_set: BTreeSet<_> = update.joined.iter().cloned().collect();
  if joined_set.len() != update.joined.len() {
    return Err(TopologyApplyError::InvalidTopology { reason: "joined contains duplicates".to_string() });
  }
  let left_set: BTreeSet<_> = update.left.iter().cloned().collect();
  if left_set.len() != update.left.len() {
    return Err(TopologyApplyError::InvalidTopology { reason: "left contains duplicates".to_string() });
  }
  let dead_set: BTreeSet<_> = update.dead.iter().cloned().collect();
  if dead_set.len() != update.dead.len() {
    return Err(TopologyApplyError::InvalidTopology { reason: "dead contains duplicates".to_string() });
  }

  if joined_set.intersection(&left_set).next().is_some()
    || joined_set.intersection(&dead_set).next().is_some()
    || left_set.intersection(&dead_set).next().is_some()
  {
    return Err(TopologyApplyError::InvalidTopology { reason: "delta sets overlap".to_string() });
  }

  let member_set: BTreeSet<_> = update.members.iter().cloned().collect();
  if member_set.len() != update.members.len() {
    return Err(TopologyApplyError::InvalidTopology { reason: "members contains duplicates".to_string() });
  }
  if left_set.iter().any(|entry| member_set.contains(entry)) || dead_set.iter().any(|entry| member_set.contains(entry))
  {
    return Err(TopologyApplyError::InvalidTopology { reason: "members contains removed entries".to_string() });
  }
  if !joined_set.is_subset(&member_set) {
    return Err(TopologyApplyError::InvalidTopology { reason: "joined not included in members".to_string() });
  }

  Ok(())
}

#[derive(Clone)]
struct ClusterStartupState {
  address: String,
}
