//! Cross data center heartbeat protocol.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CrossDcHeartbeatEvidence, CrossDcHeartbeatRequest, CrossDcHeartbeatResponse, CrossDcHeartbeatTarget,
  CrossDcHeartbeatTargetChange, DataCenter, HeartbeatProtocolState, MembershipSnapshot, NodeRecord,
};

#[cfg(test)]
#[path = "cross_dc_heartbeat_test.rs"]
mod tests;

/// Tracks cross data center heartbeat targets and delegates request state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossDcHeartbeat {
  local:             UniqueAddress,
  local_data_center: DataCenter,
  heartbeat:         HeartbeatProtocolState,
  targets:           BTreeMap<UniqueAddress, DataCenter>,
}

impl CrossDcHeartbeat {
  /// Creates cross data center heartbeat state for a local member.
  #[must_use]
  pub fn new(
    local: UniqueAddress,
    local_data_center: DataCenter,
    heartbeat_timeout_ms: u64,
    first_heartbeat_timeout_ms: u64,
  ) -> Self {
    Self {
      heartbeat: HeartbeatProtocolState::new(local.clone(), heartbeat_timeout_ms, first_heartbeat_timeout_ms),
      local,
      local_data_center,
      targets: BTreeMap::new(),
    }
  }

  /// Returns the current cross data center targets.
  #[must_use]
  pub fn targets(&self) -> Vec<CrossDcHeartbeatTarget> {
    self.targets.iter().map(|(peer, data_center)| self.target(peer.clone(), data_center.clone())).collect()
  }

  /// Updates targets from membership and reports added, removed, and retained targets.
  pub fn update_targets(&mut self, snapshot: &MembershipSnapshot) -> CrossDcHeartbeatTargetChange {
    let next = snapshot
      .entries
      .iter()
      .filter(|record| self.is_cross_dc_target(record))
      .map(|record| (record.unique_address.clone(), record.data_center.clone()))
      .collect::<BTreeMap<_, _>>();

    let added = next
      .iter()
      .filter(|(peer, data_center)| self.targets.get(peer) != Some(data_center))
      .map(|(peer, data_center)| self.target(peer.clone(), data_center.clone()))
      .collect::<Vec<_>>();
    let removed = self
      .targets
      .iter()
      .filter(|(peer, data_center)| next.get(peer) != Some(data_center))
      .map(|(peer, data_center)| self.target(peer.clone(), data_center.clone()))
      .collect::<Vec<_>>();
    let retained = next
      .iter()
      .filter(|(peer, data_center)| self.targets.get(peer) == Some(data_center))
      .map(|(peer, data_center)| self.target(peer.clone(), data_center.clone()))
      .collect::<Vec<_>>();

    for target in &removed {
      self.heartbeat.remove_peer(&target.peer);
    }
    self.targets = next;
    CrossDcHeartbeatTargetChange::new(added, removed, retained)
  }

  /// Generates cross data center heartbeat requests for current targets.
  pub fn tick(&mut self, now_ms: u64) -> Vec<CrossDcHeartbeatRequest> {
    let peers = self.targets.keys().cloned().collect::<Vec<_>>();
    self
      .heartbeat
      .tick(now_ms, &peers)
      .into_iter()
      .filter_map(|heartbeat| {
        let remote_data_center = self.targets.get(&heartbeat.to)?.clone();
        Some(CrossDcHeartbeatRequest::new(heartbeat, self.local_data_center.clone(), remote_data_center))
      })
      .collect()
  }

  /// Creates a cross data center response for a received request.
  #[must_use]
  pub fn handle_request(request: CrossDcHeartbeatRequest) -> CrossDcHeartbeatResponse {
    CrossDcHeartbeatResponse::new(
      HeartbeatProtocolState::handle_request(request.heartbeat),
      request.to_data_center,
      request.from_data_center,
    )
  }

  /// Converts a matching cross data center response into evidence.
  pub fn handle_response(
    &mut self,
    response: CrossDcHeartbeatResponse,
    now_ms: u64,
  ) -> Option<CrossDcHeartbeatEvidence> {
    if response.to_data_center != self.local_data_center {
      return None;
    }
    let remote_data_center = self.targets.get(&response.heartbeat.from)?;
    if response.from_data_center != *remote_data_center {
      return None;
    }
    let local_data_center = self.local_data_center.clone();
    let remote_data_center = remote_data_center.clone();
    self.heartbeat.handle_response(response.heartbeat, now_ms).map(|evidence| {
      CrossDcHeartbeatEvidence::new(
        evidence.observer,
        evidence.subject,
        local_data_center,
        remote_data_center,
        evidence.sequence,
        evidence.kind,
      )
    })
  }

  /// Collects cross data center timeout evidence for current targets.
  pub fn collect_timeouts(&mut self, now_ms: u64) -> Vec<CrossDcHeartbeatEvidence> {
    self
      .heartbeat
      .collect_timeouts(now_ms)
      .into_iter()
      .filter_map(|evidence| {
        let remote_data_center = self.targets.get(&evidence.subject)?.clone();
        Some(CrossDcHeartbeatEvidence::new(
          evidence.observer,
          evidence.subject,
          self.local_data_center.clone(),
          remote_data_center,
          evidence.sequence,
          evidence.kind,
        ))
      })
      .collect()
  }

  fn is_cross_dc_target(&self, record: &NodeRecord) -> bool {
    record.unique_address != self.local && record.status.is_active() && record.data_center != self.local_data_center
  }

  fn target(&self, peer: UniqueAddress, remote_data_center: DataCenter) -> CrossDcHeartbeatTarget {
    CrossDcHeartbeatTarget::new(peer, self.local_data_center.clone(), remote_data_center)
  }
}
