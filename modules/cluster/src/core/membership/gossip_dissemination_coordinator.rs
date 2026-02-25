//! Gossip dissemination coordinator for membership convergence.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};

use super::{
  GossipEvent, GossipOutbound, GossipState, MembershipDelta, MembershipTable, MembershipVersion, VectorClock,
};

#[cfg(test)]
mod tests;

/// Drives gossip diffusion, reconciliation and confirmation.
pub struct GossipDisseminationCoordinator {
  table:            MembershipTable,
  local_authority:  Option<String>,
  peers:            Vec<String>,
  peer_versions:    BTreeMap<String, MembershipVersion>,
  vector_clock:     VectorClock,
  seen_by:          BTreeSet<String>,
  state:            GossipState,
  inflight_version: MembershipVersion,
  events:           Vec<GossipEvent>,
}

impl GossipDisseminationCoordinator {
  /// Creates a new engine with known peers and optional local authority.
  #[must_use]
  pub fn new(table: MembershipTable, local_authority: Option<String>, peers: Vec<String>) -> Self {
    let current = table.version();
    let peer_versions = peers.iter().map(|p| (p.clone(), current)).collect();
    let mut vector_clock = VectorClock::new();
    for peer in peers.iter() {
      vector_clock.observe(peer, current.value());
    }
    Self {
      table,
      local_authority,
      peers,
      peer_versions,
      vector_clock,
      seen_by: BTreeSet::new(),
      state: GossipState::Confirmed,
      inflight_version: current,
      events: Vec::new(),
    }
  }

  /// Returns current state.
  #[must_use]
  pub const fn state(&self) -> GossipState {
    self.state
  }

  /// Borrows the membership table.
  #[must_use]
  pub const fn table(&self) -> &MembershipTable {
    &self.table
  }

  /// Borrows the membership table mutably.
  #[must_use]
  pub const fn table_mut(&mut self) -> &mut MembershipTable {
    &mut self.table
  }

  /// Returns authorities that have seen the inflight version.
  #[must_use]
  pub fn seen_by(&self) -> Vec<String> {
    self.seen_by.iter().cloned().collect()
  }

  /// Replaces peer list and refreshes peer versions.
  pub fn set_peers(&mut self, peers: Vec<String>) {
    let current = self.table.version();
    let mut updated_versions = BTreeMap::new();
    for peer in peers.iter() {
      let version = self.peer_versions.get(peer).copied().unwrap_or(current);
      updated_versions.insert(peer.clone(), version);
      self.vector_clock.observe(peer, version.value());
    }
    self.peers = peers;
    self.peer_versions = updated_versions;
    self.seen_by.retain(|authority| {
      self.peers.contains(authority)
        || self.local_authority.as_ref().is_some_and(|local_authority| local_authority == authority)
    });
  }

  /// Disseminates the given delta to all peers, entering Diffusing state.
  pub fn disseminate(&mut self, delta: &MembershipDelta) -> Vec<GossipOutbound> {
    let out = self.peers.iter().cloned().map(|peer| GossipOutbound::new(peer, delta.clone())).collect::<Vec<_>>();

    self.inflight_version = delta.to;
    self.state = GossipState::Diffusing;
    self.seen_by.clear();
    if let Some(local_authority) = self.local_authority.as_ref() {
      self.seen_by.insert(local_authority.clone());
    }
    let mut vector_clock = VectorClock::new();
    for peer in self.peers.iter() {
      vector_clock.observe(peer, delta.from.value());
    }
    vector_clock.observe(LOCAL_VECTOR_CLOCK_NODE, delta.to.value());
    self.vector_clock = vector_clock;
    self.events.push(GossipEvent::Disseminated { peers: self.peers.len(), version: delta.to });
    self.events.push(self.seen_changed_event());

    out
  }

  /// Handles an ack from a peer; returns the new state when it changes.
  pub fn handle_ack(&mut self, peer: &str) -> Option<GossipState> {
    self.peer_versions.insert(peer.to_string(), self.inflight_version);
    self.vector_clock.observe(peer, self.inflight_version.value());
    if self.seen_by.insert(peer.to_string()) {
      self.events.push(self.seen_changed_event());
    }

    if self.vector_clock.has_seen_all(&self.peers, self.inflight_version.value()) {
      self.state = GossipState::Confirmed;
      self.events.push(GossipEvent::Confirmed { version: self.inflight_version });
      return Some(self.state);
    }

    None
  }

  /// Requests reconciliation for a missing range reported by a peer.
  pub fn request_reconcile(&mut self, peer: &str, _from: MembershipVersion, _to: MembershipVersion) {
    self.state = GossipState::Reconciling;
    self
      .events
      .push(GossipEvent::ReconcilingRequested { peer: peer.to_string(), local_version: self.table.version() });
  }

  /// Applies an incoming delta and detects conflicts.
  pub fn apply_incoming(&mut self, delta: &MembershipDelta, peer: &str) {
    if delta.to < self.table.version() {
      self.state = GossipState::Reconciling;
      self.events.push(GossipEvent::ConflictDetected {
        peer:           peer.to_string(),
        local_version:  self.table.version(),
        remote_version: delta.to,
      });
      return;
    }

    self.table.apply_delta(delta.clone());
    self.peer_versions.insert(peer.to_string(), delta.to);
    self.inflight_version = delta.to;
    self.vector_clock.observe(peer, delta.to.value());
    if self.seen_by.insert(peer.to_string()) {
      self.events.push(self.seen_changed_event());
    }

    if self
      .peers
      .iter()
      .all(|p| self.peer_versions.get(p).copied().unwrap_or(MembershipVersion::zero()) == self.inflight_version)
    {
      self.state = GossipState::Confirmed;
      self.events.push(GossipEvent::Confirmed { version: self.inflight_version });
    } else {
      self.state = GossipState::Diffusing;
    }
  }

  /// Drains accumulated events.
  pub fn drain_events(&mut self) -> Vec<GossipEvent> {
    core::mem::take(&mut self.events)
  }

  fn seen_changed_event(&self) -> GossipEvent {
    GossipEvent::SeenChanged {
      seen_by: self.seen_by(),
      version: self.inflight_version,
      clock:   self.vector_clock.clone(),
    }
  }
}

const LOCAL_VECTOR_CLOCK_NODE: &str = "$local";
