//! Membership coordinator orchestrating gossip, failure detection, and topology updates.

#[cfg(test)]
#[path = "membership_coordinator_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use super::{
  CurrentClusterState, GossipDisseminationCoordinator, GossipEvent, MembershipCoordinatorConfig,
  MembershipCoordinatorError, MembershipCoordinatorOutcome, MembershipCoordinatorState, MembershipDelta,
  MembershipError, MembershipSnapshot, MembershipTable, MembershipVersion, NodeRecord, NodeStatus, QuarantineEntry,
  QuarantineTable, ReachabilityMatrix,
};
use crate::{
  ClusterEvent, ClusterExtensionConfig, ClusterTopology, ConfigValidation, JoinConfigCompatChecker, TopologyUpdate,
  failure_detector::{DefaultFailureDetectorRegistry, FailureDetectorRegistry},
};

/// Membership/Gossip coordinator (no_std).
pub struct MembershipCoordinator {
  config:                MembershipCoordinatorConfig,
  cluster_config:        ClusterExtensionConfig,
  state:                 MembershipCoordinatorState,
  gossip:                GossipDisseminationCoordinator,
  registry:              DefaultFailureDetectorRegistry<String>,
  quarantine:            QuarantineTable,
  reachability:          ReachabilityMatrix,
  last_cluster_state:    Option<CurrentClusterState>,
  topology_accumulator:  TopologyAccumulator,
  next_topology_emit_at: Option<TimerInstant>,
  suspect_since:         BTreeMap<String, TimerInstant>,
}

impl MembershipCoordinator {
  /// Creates a new coordinator.
  #[must_use]
  pub fn new(
    config: MembershipCoordinatorConfig,
    cluster_config: ClusterExtensionConfig,
    table: MembershipTable,
    registry: DefaultFailureDetectorRegistry<String>,
  ) -> Self {
    let local_authority = local_authority_from_config(&cluster_config);
    Self {
      config,
      cluster_config,
      state: MembershipCoordinatorState::Stopped,
      gossip: GossipDisseminationCoordinator::new(table, local_authority, Vec::new()),
      registry,
      quarantine: QuarantineTable::new(),
      reachability: ReachabilityMatrix::new(),
      last_cluster_state: None,
      topology_accumulator: TopologyAccumulator::new(),
      next_topology_emit_at: None,
      suspect_since: BTreeMap::new(),
    }
  }

  /// Returns current coordinator state.
  #[must_use]
  pub const fn state(&self) -> MembershipCoordinatorState {
    self.state
  }

  /// Starts in member mode.
  ///
  /// # Errors
  ///
  /// Returns an error when the coordinator cannot transition to member mode.
  pub fn start_member(&mut self) -> Result<(), MembershipCoordinatorError> {
    self.cluster_config.validate().map_err(MembershipCoordinatorError::Configuration)?;
    self.state = MembershipCoordinatorState::Member;
    Ok(())
  }

  /// Starts in client mode.
  ///
  /// # Errors
  ///
  /// Returns an error when the coordinator cannot transition to client mode.
  pub fn start_client(&mut self) -> Result<(), MembershipCoordinatorError> {
    self.cluster_config.validate().map_err(MembershipCoordinatorError::Configuration)?;
    self.state = MembershipCoordinatorState::Client;
    Ok(())
  }

  /// Stops the coordinator.
  ///
  /// # Errors
  ///
  /// Returns an error when the coordinator cannot stop.
  pub fn stop(&mut self) -> Result<(), MembershipCoordinatorError> {
    self.state = MembershipCoordinatorState::Stopped;
    self.suspect_since.clear();
    self.topology_accumulator.clear();
    self.reachability = ReachabilityMatrix::new();
    self.last_cluster_state = None;
    Ok(())
  }

  /// Returns current membership snapshot.
  #[must_use]
  pub fn snapshot(&self) -> MembershipSnapshot {
    if self.state == MembershipCoordinatorState::Stopped {
      return MembershipSnapshot::new(MembershipVersion::zero(), Vec::new());
    }
    let snapshot = self.gossip.table().snapshot();
    MembershipSnapshot::new_with_reachability(snapshot.version, snapshot.entries, self.reachability.snapshot())
  }

  /// Returns current quarantine snapshot.
  #[must_use]
  pub fn quarantine_snapshot(&self) -> Vec<QuarantineEntry> {
    if self.state == MembershipCoordinatorState::Stopped {
      return Vec::new();
    }
    self.quarantine.snapshot()
  }

  /// Handles a join request.
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped, or
  /// [`MembershipCoordinatorError::InvalidState`] in client mode.
  pub fn handle_join(
    &mut self,
    node_id: String,
    authority: String,
    joining_config: &ClusterExtensionConfig,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_member()?;

    if self.quarantine.contains(&authority) {
      let reason = self
        .quarantine
        .snapshot()
        .into_iter()
        .find(|entry| entry.authority == authority)
        .map(|entry| entry.reason)
        .unwrap_or_else(|| "quarantined".to_string());
      return Err(MembershipCoordinatorError::Membership(MembershipError::Quarantined { authority, reason }));
    }

    joining_config.validate().map_err(MembershipCoordinatorError::Configuration)?;

    if let ConfigValidation::Incompatible { reason } = self.cluster_config.check_join_compatibility(joining_config) {
      return Err(MembershipCoordinatorError::Membership(MembershipError::IncompatibleConfig { reason }));
    }

    let before = self.gossip.table().record(&authority).map(|r| r.status);
    let delta = self
      .gossip
      .table_mut()
      .try_join(
        node_id.clone(),
        authority.clone(),
        joining_config.app_version().to_string(),
        joining_config.roles().to_vec(),
      )
      .map_err(MembershipCoordinatorError::Membership)?;

    let changed = delta.from != delta.to;
    let membership_events = self.gossip.table_mut().drain_events();
    let mut outcome = MembershipCoordinatorOutcome { membership_events, ..Default::default() };

    if changed {
      self.update_suspect_tracking(&authority, NodeStatus::Joining, now);
      self.topology_accumulator.joined.insert(authority.clone());
      self.refresh_peers();
      if self.config.gossip_enabled {
        outcome.gossip_outbound = self.gossip.disseminate(&delta);
      }
      let from = before.unwrap_or(NodeStatus::Removed);
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id,
        authority,
        from,
        to: NodeStatus::Joining,
        observed_at: now,
      });
    }

    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  /// Handles a leave request.
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped, or
  /// [`MembershipCoordinatorError::InvalidState`] in client mode.
  pub fn handle_leave(
    &mut self,
    authority: &str,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_member()?;

    let before = self.gossip.table().record(authority).map(|r| r.status);
    let first_delta = self.gossip.table_mut().mark_left(authority).map_err(MembershipCoordinatorError::Membership)?;
    let first_to = self.gossip.table().record(authority).map(|record| record.status);

    let mut deltas = vec![first_delta];
    let mut second_to = None;
    if first_to == Some(NodeStatus::Exiting) {
      let second_delta =
        self.gossip.table_mut().mark_left(authority).map_err(MembershipCoordinatorError::Membership)?;
      second_to = self.gossip.table().record(authority).map(|record| record.status);
      deltas.push(second_delta);
    }

    let membership_events = self.gossip.table_mut().drain_events();
    let mut outcome = MembershipCoordinatorOutcome { membership_events, ..Default::default() };
    if self.gossip.table().record(authority).map(|record| record.status) == Some(NodeStatus::Removed) {
      self.topology_accumulator.left.insert(authority.to_string());
    }
    if let Some(status) = self.gossip.table().record(authority).map(|record| record.status) {
      self.update_suspect_tracking(authority, status, now);
    }
    self.refresh_peers();

    if self.config.gossip_enabled {
      for delta in deltas.iter() {
        outcome.gossip_outbound.extend(self.gossip.disseminate(delta));
      }
    }

    if let Some(from) = before
      && let Some(to) = first_to
      && from != to
      && let Some(record) = self.gossip.table().record(authority)
    {
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id: record.node_id.clone(),
        authority: record.authority.clone(),
        from,
        to,
        observed_at: now,
      });
    }

    if let Some(to) = second_to
      && to != NodeStatus::Exiting
      && let Some(record) = self.gossip.table().record(authority)
    {
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id: record.node_id.clone(),
        authority: record.authority.clone(),
        from: NodeStatus::Exiting,
        to,
        observed_at: now,
      });
    }

    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  /// Handles heartbeat receipt.
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped.
  pub fn handle_heartbeat(
    &mut self,
    authority: &str,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_started()?;

    if self.gossip.table().record(authority).is_none() {
      return Err(MembershipCoordinatorError::Membership(MembershipError::UnknownAuthority {
        authority: authority.to_string(),
      }));
    }

    let mut outcome = MembershipCoordinatorOutcome::default();
    let now_ms = to_millis(now);
    let authority_key = authority.to_string();

    let status = self.gossip.table().record(authority).map(|record| record.status);
    if let Some(status) = status
      && matches!(status, NodeStatus::Joining | NodeStatus::WeaklyUp)
    {
      let next_status = if status == NodeStatus::Joining { NodeStatus::WeaklyUp } else { NodeStatus::Up };
      let delta = if status == NodeStatus::Joining {
        self.gossip.table_mut().mark_weakly_up(authority).map_err(MembershipCoordinatorError::Membership)?
      } else {
        self.gossip.table_mut().mark_up(authority).map_err(MembershipCoordinatorError::Membership)?
      };
      if let Some(delta) = delta {
        if self.config.gossip_enabled {
          outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
        }
        self.emit_status_change(authority, status, next_status, now, &mut outcome);
      }
    }

    let was_suspect = self.suspect_since.contains_key(&authority_key);
    self.registry.heartbeat(&authority_key, now_ms);

    if was_suspect {
      self.apply_reachable(&authority_key, now, &mut outcome)?;
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  /// Handles incoming gossip delta.
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped.
  pub fn handle_gossip_delta(
    &mut self,
    peer: &str,
    delta: &MembershipDelta,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_started()?;

    let snapshot = self.gossip.table().snapshot();
    let mut previous = BTreeMap::new();
    let mut incoming_keys = BTreeSet::new();
    for record in delta.entries.iter() {
      incoming_keys.insert(record.unique_address.to_string());
      previous.insert(
        record.unique_address.to_string(),
        snapshot.entries.iter().find(|entry| entry.unique_address == record.unique_address).map(|entry| entry.status),
      );
    }
    for record in snapshot.entries.iter() {
      previous.entry(record.unique_address.to_string()).or_insert(Some(record.status));
    }

    let superseded = self.gossip.apply_incoming(delta, peer);
    self.refresh_peers();

    let mut outcome = MembershipCoordinatorOutcome::default();
    for record in delta.entries.iter() {
      let before = previous.get(&record.unique_address.to_string()).copied().flatten();
      self.register_membership_change(record, before, now, &mut outcome);
    }
    for record in superseded.iter().filter(|record| !incoming_keys.contains(&record.unique_address.to_string())) {
      let before = previous.get(&record.unique_address.to_string()).copied().flatten();
      self.register_membership_change(record, before, now, &mut outcome);
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  /// Handles quarantine event from transport.
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped.
  pub fn handle_quarantine(
    &mut self,
    authority: String,
    reason: String,
    now: TimerInstant,
  ) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_started()?;

    let mut outcome = MembershipCoordinatorOutcome::default();
    let event = self.quarantine.quarantine(authority.clone(), reason.clone(), now, self.config.quarantine_ttl);
    outcome.quarantine_events.push(event);
    outcome.member_events.push(ClusterEvent::MemberQuarantined { authority, reason, observed_at: now });
    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  /// Polls periodic tasks (failure detection, topology emission).
  ///
  /// # Errors
  ///
  /// Returns [`MembershipCoordinatorError::NotStarted`] when stopped.
  pub fn poll(&mut self, now: TimerInstant) -> Result<MembershipCoordinatorOutcome, MembershipCoordinatorError> {
    self.ensure_started()?;

    let mut outcome = MembershipCoordinatorOutcome::default();
    let now_ms = to_millis(now);

    self.detect_suspects(now_ms, now, &mut outcome)?;

    let cleared = self.quarantine.poll_expired(now);
    for event in cleared.into_iter() {
      outcome.quarantine_events.push(event);
    }

    if let Some(event) = self.emit_topology_if_due(now) {
      outcome.topology_event = Some(event);
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
    self.collect_gossip_and_state_events(now, &mut outcome);
    Ok(outcome)
  }

  fn detect_suspects(
    &mut self,
    now_ms: u64,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) -> Result<(), MembershipCoordinatorError> {
    let active_authorities: Vec<String> = self
      .gossip
      .table()
      .snapshot()
      .entries
      .iter()
      .filter(|record| record.status.is_active())
      .map(|record| record.authority.clone())
      .collect();

    for authority in active_authorities {
      if self.suspect_since.contains_key(&authority) {
        continue;
      }
      if !self.registry.is_monitoring(&authority) {
        continue;
      }
      if !self.registry.is_available(&authority, now_ms) {
        self.apply_suspect(&authority, now, outcome)?;
      }
    }
    Ok(())
  }

  fn apply_suspect(
    &mut self,
    authority: &str,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) -> Result<(), MembershipCoordinatorError> {
    // mark_suspect は Up | Joining → Suspect の遷移を行うため、遷移前の状態を保存しておく
    let previous_status = self.gossip.table().record(authority).map(|r| r.status);
    if let Some(delta) =
      self.gossip.table_mut().mark_suspect(authority).map_err(MembershipCoordinatorError::Membership)?
    {
      self.suspect_since.entry(authority.to_string()).or_insert(now);
      if self.config.gossip_enabled {
        outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
      }
      if let Some(record) = self.gossip.table().record(authority).cloned() {
        let from = previous_status.unwrap_or(NodeStatus::Up);
        self.record_unreachable(&record);
        self.emit_status_change(authority, from, record.status, now, outcome);
        Self::emit_unreachable_event(&record, now, outcome);
      }
    }
    Ok(())
  }

  fn apply_reachable(
    &mut self,
    authority: &str,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) -> Result<(), MembershipCoordinatorError> {
    if let Some(delta) = self.gossip.table_mut().mark_up(authority).map_err(MembershipCoordinatorError::Membership)? {
      self.suspect_since.remove(authority);
      if self.config.gossip_enabled {
        outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
      }
      if let Some(record) = self.gossip.table().record(authority).cloned() {
        self.record_reachable(&record);
        self.emit_status_change(authority, NodeStatus::Suspect, record.status, now, outcome);
        Self::emit_reachable_event(&record, now, outcome);
      }
    }
    Ok(())
  }

  fn ensure_started(&self) -> Result<(), MembershipCoordinatorError> {
    if self.state == MembershipCoordinatorState::Stopped {
      return Err(MembershipCoordinatorError::NotStarted);
    }
    Ok(())
  }

  fn ensure_member(&self) -> Result<(), MembershipCoordinatorError> {
    self.ensure_started()?;
    if self.state == MembershipCoordinatorState::Client {
      return Err(MembershipCoordinatorError::InvalidState { state: self.state });
    }
    Ok(())
  }

  fn refresh_peers(&mut self) {
    let peers = self
      .gossip
      .table()
      .snapshot()
      .entries
      .iter()
      .filter(|record| record.status.is_active())
      .map(|record| record.authority.clone())
      .collect::<Vec<_>>();
    self.gossip.set_peers(peers);
  }

  fn register_membership_change(
    &mut self,
    record: &NodeRecord,
    before: Option<NodeStatus>,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) {
    let status = record.status;
    self.update_suspect_tracking(record.authority.as_str(), status, now);
    match status {
      | NodeStatus::Joining | NodeStatus::WeaklyUp | NodeStatus::Up | NodeStatus::Suspect => {
        if before.is_none() || matches!(before, Some(NodeStatus::Removed | NodeStatus::Dead)) {
          self.topology_accumulator.joined.insert(record.authority.clone());
        }
      },
      | NodeStatus::Removed => {
        self.topology_accumulator.left.insert(record.authority.clone());
      },
      | NodeStatus::Dead => {
        self.topology_accumulator.dead.insert(record.authority.clone());
        let reason = "gossip-dead".to_string();
        let event =
          self.quarantine.quarantine(record.authority.clone(), reason.clone(), now, self.config.quarantine_ttl);
        outcome.quarantine_events.push(event);
        outcome.member_events.push(ClusterEvent::MemberQuarantined {
          authority: record.authority.clone(),
          reason,
          observed_at: now,
        });
      },
      | NodeStatus::Leaving | NodeStatus::Exiting | NodeStatus::PreparingForShutdown | NodeStatus::ReadyForShutdown => {
      },
    }

    if let Some(from) = before
      && from != status
    {
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id: record.node_id.clone(),
        authority: record.authority.clone(),
        from,
        to: status,
        observed_at: now,
      });
      if status == NodeStatus::Suspect {
        Self::emit_unreachable_event(record, now, outcome);
      } else if from == NodeStatus::Suspect && status == NodeStatus::Up {
        Self::emit_reachable_event(record, now, outcome);
      }
      // shutdown 系専用イベントの併発（順序固定: MemberStatusChanged の後）
      match status {
        | NodeStatus::PreparingForShutdown => {
          outcome.member_events.push(ClusterEvent::MemberPreparingForShutdown {
            node_id:     record.node_id.clone(),
            authority:   record.authority.clone(),
            observed_at: now,
          });
        },
        | NodeStatus::ReadyForShutdown => {
          outcome.member_events.push(ClusterEvent::MemberReadyForShutdown {
            node_id:     record.node_id.clone(),
            authority:   record.authority.clone(),
            observed_at: now,
          });
        },
        | _ => {},
      }
    }
  }

  fn update_suspect_tracking(&mut self, authority: &str, status: NodeStatus, now: TimerInstant) {
    if status == NodeStatus::Suspect {
      self.suspect_since.entry(authority.to_string()).or_insert(now);
    } else {
      self.suspect_since.remove(authority);
    }
  }

  fn record_unreachable(&mut self, subject: &NodeRecord) {
    self.reachability.unreachable(self.local_unique_address(), subject.unique_address.clone());
  }

  fn record_reachable(&mut self, subject: &NodeRecord) {
    self.reachability.reachable(self.local_unique_address(), subject.unique_address.clone());
  }

  fn local_unique_address(&self) -> UniqueAddress {
    local_authority_from_config(&self.cluster_config)
      .map_or_else(default_local_unique_address, unique_address_from_authority)
  }

  fn emit_status_change(
    &self,
    authority: &str,
    from: NodeStatus,
    to: NodeStatus,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) {
    if let Some(record) = self.gossip.table().record(authority) {
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id: record.node_id.clone(),
        authority: record.authority.clone(),
        from,
        to,
        observed_at: now,
      });
      // shutdown 系専用イベントの併発（順序固定: MemberStatusChanged の後）
      match to {
        | NodeStatus::PreparingForShutdown => {
          outcome.member_events.push(ClusterEvent::MemberPreparingForShutdown {
            node_id:     record.node_id.clone(),
            authority:   record.authority.clone(),
            observed_at: now,
          });
        },
        | NodeStatus::ReadyForShutdown => {
          outcome.member_events.push(ClusterEvent::MemberReadyForShutdown {
            node_id:     record.node_id.clone(),
            authority:   record.authority.clone(),
            observed_at: now,
          });
        },
        | _ => {},
      }
    }
  }

  fn emit_unreachable_event(record: &NodeRecord, now: TimerInstant, outcome: &mut MembershipCoordinatorOutcome) {
    outcome.member_events.push(ClusterEvent::UnreachableMember {
      node_id:     record.node_id.clone(),
      authority:   record.authority.clone(),
      observed_at: now,
    });
  }

  fn emit_reachable_event(record: &NodeRecord, now: TimerInstant, outcome: &mut MembershipCoordinatorOutcome) {
    outcome.member_events.push(ClusterEvent::ReachableMember {
      node_id:     record.node_id.clone(),
      authority:   record.authority.clone(),
      observed_at: now,
    });
  }

  fn collect_gossip_and_state_events(&mut self, now: TimerInstant, outcome: &mut MembershipCoordinatorOutcome) {
    self.collect_seen_changed_events(now, outcome);
    self.collect_current_cluster_state_event(now, outcome);
  }

  fn collect_seen_changed_events(&mut self, now: TimerInstant, outcome: &mut MembershipCoordinatorOutcome) {
    let events = self.gossip.drain_events();
    for event in events {
      if let GossipEvent::SeenChanged { seen_by, version, .. } = event {
        outcome.member_events.push(ClusterEvent::SeenChanged { seen_by, version, observed_at: now });
      }
    }
  }

  fn current_cluster_state(&self) -> CurrentClusterState {
    let snapshot = self.gossip.table().snapshot();
    let members = snapshot
      .entries
      .iter()
      .filter(|record| !matches!(record.status, NodeStatus::Removed | NodeStatus::Dead))
      .cloned()
      .collect::<Vec<_>>();
    let unreachable = members.iter().filter(|record| record.status == NodeStatus::Suspect).cloned().collect::<Vec<_>>();
    let leader_members =
      members.iter().filter(|record| is_leader_eligible_status(record.status)).cloned().collect::<Vec<_>>();
    let leader = oldest_authority(&leader_members);
    let role_leader = role_leaders(&members);
    let seen_by = self.gossip.seen_by();
    CurrentClusterState::new_with_reachability(
      members,
      unreachable,
      seen_by,
      leader,
      role_leader,
      self.reachability.snapshot(),
    )
  }

  fn collect_current_cluster_state_event(&mut self, now: TimerInstant, outcome: &mut MembershipCoordinatorOutcome) {
    let state = self.current_cluster_state();
    if self.last_cluster_state.as_ref().is_some_and(|last_state| last_state == &state) {
      return;
    }
    self.last_cluster_state = Some(state.clone());
    outcome.member_events.push(ClusterEvent::CurrentClusterState { state, observed_at: now });
  }

  fn emit_topology_if_due(&mut self, now: TimerInstant) -> Option<ClusterEvent> {
    if self.topology_accumulator.is_empty() {
      return None;
    }

    let next_due = match self.next_topology_emit_at {
      | Some(deadline) => deadline,
      | None => {
        let deadline = add_duration(now, self.config.topology_emit_interval);
        self.next_topology_emit_at = Some(deadline);
        deadline
      },
    };

    if now < next_due {
      return None;
    }

    let joined = self.topology_accumulator.joined_sorted();
    let left = self.topology_accumulator.left_sorted();
    let dead = self.topology_accumulator.dead_sorted();
    let hash = self.gossip.table().version().value();
    let topology = ClusterTopology::new(hash, joined.clone(), left.clone(), dead.clone());
    let members = self
      .gossip
      .table()
      .snapshot()
      .entries
      .into_iter()
      .filter(|record| {
        !matches!(record.status, NodeStatus::Leaving | NodeStatus::Exiting | NodeStatus::Removed | NodeStatus::Dead)
      })
      .map(|record| record.authority)
      .collect::<Vec<_>>();
    let update = TopologyUpdate::new(topology, members, joined, left, dead, Vec::new(), now);

    self.topology_accumulator.clear();
    self.next_topology_emit_at = Some(add_duration(now, self.config.topology_emit_interval));

    Some(ClusterEvent::TopologyUpdated { update })
  }
}

struct TopologyAccumulator {
  joined: BTreeSet<String>,
  left:   BTreeSet<String>,
  dead:   BTreeSet<String>,
}

impl TopologyAccumulator {
  const fn new() -> Self {
    Self { joined: BTreeSet::new(), left: BTreeSet::new(), dead: BTreeSet::new() }
  }

  fn is_empty(&self) -> bool {
    self.joined.is_empty() && self.left.is_empty() && self.dead.is_empty()
  }

  fn clear(&mut self) {
    self.joined.clear();
    self.left.clear();
    self.dead.clear();
  }

  fn joined_sorted(&self) -> Vec<String> {
    self.joined.iter().cloned().collect()
  }

  fn left_sorted(&self) -> Vec<String> {
    self.left.iter().cloned().collect()
  }

  fn dead_sorted(&self) -> Vec<String> {
    self.dead.iter().cloned().collect()
  }
}

fn oldest_authority(records: &[NodeRecord]) -> Option<String> {
  let mut oldest: Option<&NodeRecord> = None;
  for record in records {
    oldest = match oldest {
      | Some(current) if !record.is_older_than(current) => Some(current),
      | _ => Some(record),
    };
  }
  oldest.map(|record| record.authority.clone())
}

fn role_leaders(records: &[NodeRecord]) -> BTreeMap<String, Option<String>> {
  let mut role_records: BTreeMap<String, Option<NodeRecord>> = BTreeMap::new();
  for record in records {
    for role in record.roles.iter() {
      let entry = role_records.entry(role.clone()).or_insert(None);
      if !is_leader_eligible_status(record.status) {
        continue;
      }
      let replace = match entry {
        | Some(current) => record.is_older_than(current),
        | None => true,
      };
      if replace {
        *entry = Some(record.clone());
      }
    }
  }
  role_records.into_iter().map(|(role, record)| (role, record.map(|record| record.authority))).collect()
}

fn local_authority_from_config(cluster_config: &ClusterExtensionConfig) -> Option<String> {
  if cluster_config.advertised_address().is_empty() {
    None
  } else {
    Some(String::from(cluster_config.advertised_address()))
  }
}

fn unique_address_from_authority(authority: String) -> UniqueAddress {
  let (host, port) = authority_host_port(authority);
  UniqueAddress::new(Address::new("fraktor-cluster", host, port), 1)
}

fn default_local_unique_address() -> UniqueAddress {
  UniqueAddress::new(Address::new("fraktor-cluster", "local", 0), 1)
}

fn authority_host_port(authority: String) -> (String, u16) {
  if let Some((host, port_text)) = authority.rsplit_once(':')
    && let Ok(port) = port_text.parse::<u16>()
  {
    (host.to_string(), port)
  } else {
    (authority, 0)
  }
}

const fn is_leader_eligible_status(status: NodeStatus) -> bool {
  matches!(
    status,
    NodeStatus::Up | NodeStatus::Leaving | NodeStatus::PreparingForShutdown | NodeStatus::ReadyForShutdown
  )
}

fn to_millis(now: TimerInstant) -> u64 {
  let resolution_ns = now.resolution().as_nanos().max(1);
  let ticks = now.ticks().saturating_mul(u64::try_from(resolution_ns).unwrap_or(u64::MAX));
  ticks / 1_000_000
}

fn add_duration(now: TimerInstant, duration: Duration) -> TimerInstant {
  if duration.is_zero() {
    return now;
  }
  let resolution_ns = now.resolution().as_nanos().max(1);
  let duration_ns = duration.as_nanos();
  let mut ticks = duration_ns / resolution_ns;
  if ticks == 0 {
    ticks = 1;
  }
  let ticks = u64::try_from(ticks).unwrap_or(u64::MAX);
  now.saturating_add_ticks(ticks)
}
