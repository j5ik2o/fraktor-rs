//! Gossip convergence engine coordinating membership dissemination.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};

use crate::core::{
  gossip_event::GossipEvent, gossip_outbound::GossipOutbound, gossip_state::GossipState,
  membership_delta::MembershipDelta, membership_table::MembershipTable, membership_version::MembershipVersion,
};

#[cfg(test)]
mod tests;

/// Drives gossip diffusion, reconciliation and confirmation.
pub struct GossipEngine {
  table:            MembershipTable,
  peers:            Vec<String>,
  peer_versions:    BTreeMap<String, MembershipVersion>,
  state:            GossipState,
  inflight_version: MembershipVersion,
  outstanding:      BTreeSet<String>,
  events:           Vec<GossipEvent>,
}

impl GossipEngine {
  /// Creates a new engine with known peers.
  #[must_use]
  pub fn new(table: MembershipTable, peers: Vec<String>) -> Self {
    let current = table.version();
    let peer_versions = peers.iter().map(|p| (p.clone(), current)).collect();
    Self {
      table,
      peers,
      peer_versions,
      state: GossipState::Confirmed,
      inflight_version: current,
      outstanding: BTreeSet::new(),
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

  /// Disseminates the given delta to all peers, entering Diffusing state.
  pub fn disseminate(&mut self, delta: &MembershipDelta) -> Vec<GossipOutbound> {
    let out = self.peers.iter().cloned().map(|peer| GossipOutbound::new(peer, delta.clone())).collect::<Vec<_>>();

    self.inflight_version = delta.to;
    self.state = GossipState::Diffusing;
    self.outstanding = self.peers.iter().cloned().collect();
    self.events.push(GossipEvent::Disseminated { peers: self.outstanding.len(), version: delta.to });

    out
  }

  /// Handles an ack from a peer; returns the new state when it changes.
  pub fn handle_ack(&mut self, peer: &str) -> Option<GossipState> {
    self.outstanding.remove(peer);
    self.peer_versions.insert(peer.to_string(), self.inflight_version);

    if self.outstanding.is_empty() {
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
}
