//! Membership coordinator orchestrating gossip, failure detection, and topology updates.

#[cfg(test)]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};
use core::{marker::PhantomData, time::Duration};

use fraktor_remote_rs::core::failure_detector::phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorEffect};
use fraktor_utils_rs::core::time::TimerInstant;

use super::{
  GossipDisseminationCoordinator, MembershipCoordinatorConfig, MembershipCoordinatorError,
  MembershipCoordinatorOutcome, MembershipCoordinatorState, MembershipDelta, MembershipError, MembershipSnapshot,
  MembershipTable, MembershipVersion, NodeStatus, QuarantineEntry, QuarantineTable,
};
use crate::core::{ClusterEvent, ClusterTopology, TopologyUpdate};

/// Membership/Gossip coordinator (no_std).
pub struct MembershipCoordinatorGeneric<TB: fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox + 'static> {
  config:                MembershipCoordinatorConfig,
  state:                 MembershipCoordinatorState,
  gossip:                GossipDisseminationCoordinator,
  detector:              PhiFailureDetector,
  quarantine:            QuarantineTable,
  topology_accumulator:  TopologyAccumulator,
  next_topology_emit_at: Option<TimerInstant>,
  suspect_since:         BTreeMap<String, TimerInstant>,
  _marker:               PhantomData<TB>,
}

impl<TB: fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox + 'static> MembershipCoordinatorGeneric<TB> {
  /// Creates a new coordinator.
  #[must_use]
  pub fn new(config: MembershipCoordinatorConfig, table: MembershipTable, detector: PhiFailureDetector) -> Self {
    Self {
      config,
      state: MembershipCoordinatorState::Stopped,
      gossip: GossipDisseminationCoordinator::new(table, Vec::new()),
      detector,
      quarantine: QuarantineTable::new(),
      topology_accumulator: TopologyAccumulator::new(),
      next_topology_emit_at: None,
      suspect_since: BTreeMap::new(),
      _marker: PhantomData,
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
  pub const fn start_member(&mut self) -> Result<(), MembershipCoordinatorError> {
    self.state = MembershipCoordinatorState::Member;
    Ok(())
  }

  /// Starts in client mode.
  ///
  /// # Errors
  ///
  /// Returns an error when the coordinator cannot transition to client mode.
  pub const fn start_client(&mut self) -> Result<(), MembershipCoordinatorError> {
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
    Ok(())
  }

  /// Returns current membership snapshot.
  #[must_use]
  pub fn snapshot(&self) -> MembershipSnapshot {
    if self.state == MembershipCoordinatorState::Stopped {
      return MembershipSnapshot::new(MembershipVersion::zero(), Vec::new());
    }
    self.gossip.table().snapshot()
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

    let before = self.gossip.table().record(&authority).map(|r| r.status);
    let delta = self
      .gossip
      .table_mut()
      .try_join(node_id.clone(), authority.clone())
      .map_err(MembershipCoordinatorError::Membership)?;

    let changed = delta.from != delta.to;
    let membership_events = self.gossip.table_mut().drain_events();
    let mut outcome = MembershipCoordinatorOutcome { membership_events, ..Default::default() };

    if changed {
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
    let delta = self.gossip.table_mut().mark_left(authority).map_err(MembershipCoordinatorError::Membership)?;

    let membership_events = self.gossip.table_mut().drain_events();
    let mut outcome = MembershipCoordinatorOutcome { membership_events, ..Default::default() };
    self.topology_accumulator.left.insert(authority.to_string());
    self.refresh_peers();

    if self.config.gossip_enabled {
      outcome.gossip_outbound = self.gossip.disseminate(&delta);
    }

    if let Some(from) = before
      && let Some(record) = self.gossip.table().record(authority)
    {
      outcome.member_events.push(ClusterEvent::MemberStatusChanged {
        node_id: record.node_id.clone(),
        authority: record.authority.clone(),
        from,
        to: record.status,
        observed_at: now,
      });
    }

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

    let status = self.gossip.table().record(authority).map(|record| record.status);
    if let Some(status) = status
      && status == NodeStatus::Joining
      && let Some(delta) = self.gossip.table_mut().mark_up(authority).map_err(MembershipCoordinatorError::Membership)?
    {
      if self.config.gossip_enabled {
        outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
      }
      self.emit_status_change(authority, status, NodeStatus::Up, now, &mut outcome);
    }

    if let Some(effect) = self.detector.record_heartbeat(authority, now_ms) {
      self.apply_detector_effect(effect, now, &mut outcome)?;
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
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

    let mut previous = BTreeMap::new();
    for record in delta.entries.iter() {
      previous.insert(record.authority.clone(), self.gossip.table().record(&record.authority).map(|r| r.status));
    }

    self.gossip.apply_incoming(delta, peer);
    self.refresh_peers();

    let mut outcome = MembershipCoordinatorOutcome::default();
    for record in delta.entries.iter() {
      let before = previous.get(&record.authority).copied().flatten();
      self.register_membership_change(record, before, now, &mut outcome);
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
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

    let effects = self.detector.poll(now_ms);
    for effect in effects {
      self.apply_detector_effect(effect, now, &mut outcome)?;
    }

    self.handle_suspect_timeouts(now, &mut outcome)?;

    let cleared = self.quarantine.poll_expired(now);
    for event in cleared.into_iter() {
      outcome.quarantine_events.push(event);
    }

    if let Some(event) = self.emit_topology_if_due(now) {
      outcome.topology_event = Some(event);
    }

    outcome.membership_events = self.gossip.table_mut().drain_events();
    Ok(outcome)
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

  fn apply_detector_effect(
    &mut self,
    effect: PhiFailureDetectorEffect,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) -> Result<(), MembershipCoordinatorError> {
    match effect {
      | PhiFailureDetectorEffect::Suspect { authority, .. } => {
        if let Some(delta) =
          self.gossip.table_mut().mark_suspect(&authority).map_err(MembershipCoordinatorError::Membership)?
        {
          self.suspect_since.entry(authority.clone()).or_insert(now);
          if self.config.gossip_enabled {
            outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
          }
          if let Some(record) = self.gossip.table().record(&authority) {
            self.emit_status_change(&authority, NodeStatus::Up, record.status, now, outcome);
          }
        }
      },
      | PhiFailureDetectorEffect::Reachable { authority } => {
        if let Some(delta) =
          self.gossip.table_mut().mark_up(&authority).map_err(MembershipCoordinatorError::Membership)?
        {
          self.suspect_since.remove(&authority);
          if self.config.gossip_enabled {
            outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
          }
          if let Some(record) = self.gossip.table().record(&authority) {
            self.emit_status_change(&authority, NodeStatus::Suspect, record.status, now, outcome);
          }
        }
      },
    }
    Ok(())
  }

  fn handle_suspect_timeouts(
    &mut self,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) -> Result<(), MembershipCoordinatorError> {
    let timeout = self.config.suspect_timeout;
    let expired = self
      .suspect_since
      .iter()
      .filter(|(_, since)| is_expired(**since, now, timeout))
      .map(|(authority, _)| authority.clone())
      .collect::<Vec<_>>();
    for authority in expired {
      self.suspect_since.remove(&authority);
      if let Some(delta) =
        self.gossip.table_mut().mark_dead(&authority).map_err(MembershipCoordinatorError::Membership)?
      {
        self.topology_accumulator.dead.insert(authority.clone());
        if self.config.gossip_enabled {
          outcome.gossip_outbound.extend(self.gossip.disseminate(&delta));
        }
        if let Some(record) = self.gossip.table().record(&authority) {
          self.emit_status_change(&authority, NodeStatus::Suspect, record.status, now, outcome);
        }
        let reason = "suspect timeout".to_string();
        let event = self.quarantine.quarantine(authority.clone(), reason.clone(), now, self.config.quarantine_ttl);
        outcome.quarantine_events.push(event);
        outcome.member_events.push(ClusterEvent::MemberQuarantined { authority, reason, observed_at: now });
      }
    }
    Ok(())
  }

  fn register_membership_change(
    &mut self,
    record: &super::NodeRecord,
    before: Option<NodeStatus>,
    now: TimerInstant,
    outcome: &mut MembershipCoordinatorOutcome,
  ) {
    let status = record.status;
    match status {
      | NodeStatus::Joining | NodeStatus::Up | NodeStatus::Suspect => {
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
      | NodeStatus::Leaving => {},
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
    }
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
    }
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
      .filter(|record| !matches!(record.status, NodeStatus::Removed | NodeStatus::Dead))
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

fn to_millis(now: TimerInstant) -> u64 {
  let resolution_ns = now.resolution().as_nanos().max(1);
  let ticks = now.ticks().saturating_mul(u64::try_from(resolution_ns).unwrap_or(u64::MAX));
  ticks / 1_000_000
}

fn is_expired(since: TimerInstant, now: TimerInstant, timeout: Duration) -> bool {
  let deadline = add_duration(since, timeout);
  now >= deadline
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
