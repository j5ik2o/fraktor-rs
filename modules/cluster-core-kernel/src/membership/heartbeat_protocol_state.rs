//! Dedicated cluster heartbeat protocol state.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{HeartbeatEvidence, HeartbeatEvidenceKind, HeartbeatRequest, HeartbeatResponse};

#[cfg(test)]
#[path = "heartbeat_protocol_state_test.rs"]
mod tests;

/// Tracks heartbeat sequence numbers, pending requests, and timeout evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatProtocolState {
  local: UniqueAddress,
  heartbeat_timeout_ms: u64,
  first_heartbeat_timeout_ms: u64,
  next_sequences: BTreeMap<UniqueAddress, u64>,
  pending: BTreeMap<PendingHeartbeatKey, PendingHeartbeat>,
  has_success: BTreeMap<UniqueAddress, bool>,
  first_miss_reported: BTreeMap<UniqueAddress, bool>,
}

impl HeartbeatProtocolState {
  /// Creates heartbeat protocol state for a local member.
  #[must_use]
  pub const fn new(local: UniqueAddress, heartbeat_timeout_ms: u64, first_heartbeat_timeout_ms: u64) -> Self {
    Self {
      local,
      heartbeat_timeout_ms,
      first_heartbeat_timeout_ms,
      next_sequences: BTreeMap::new(),
      pending: BTreeMap::new(),
      has_success: BTreeMap::new(),
      first_miss_reported: BTreeMap::new(),
    }
  }

  /// Generates heartbeat requests for the provided peers.
  pub fn tick(&mut self, now_ms: u64, peers: &[UniqueAddress]) -> Vec<HeartbeatRequest> {
    let mut requests = Vec::new();
    for peer in peers {
      let sequence = self.next_sequence(peer.clone());
      let waiting_for_first_result = !self.has_success.get(peer).copied().unwrap_or(false)
        && !self.first_miss_reported.get(peer).copied().unwrap_or(false);
      let timeout_ms =
        if waiting_for_first_result { self.first_heartbeat_timeout_ms } else { self.heartbeat_timeout_ms };
      let request = HeartbeatRequest::new(self.local.clone(), peer.clone(), sequence, now_ms + timeout_ms);
      self.pending.insert(PendingHeartbeatKey { peer: peer.clone(), sequence }, PendingHeartbeat {
        sent_at_ms:  now_ms,
        deadline_ms: now_ms + timeout_ms,
      });
      requests.push(request);
    }
    requests
  }

  /// Creates a response for a received heartbeat request.
  #[must_use]
  pub fn handle_request(request: HeartbeatRequest) -> HeartbeatResponse {
    HeartbeatResponse::new(request.to, request.from, request.sequence)
  }

  /// Converts a matching heartbeat response into reachable evidence.
  pub fn handle_response(&mut self, response: HeartbeatResponse, now_ms: u64) -> Option<HeartbeatEvidence> {
    if response.to != self.local {
      return None;
    }
    let key = PendingHeartbeatKey { peer: response.from.clone(), sequence: response.sequence };
    let pending = *self.pending.get(&key)?;
    if pending.deadline_ms < now_ms {
      return None;
    }
    self.pending.retain(|pending_key, _| pending_key.peer != response.from || pending_key.sequence > response.sequence);
    self.has_success.insert(response.from.clone(), true);
    Some(HeartbeatEvidence::new(
      self.local.clone(),
      response.from,
      response.sequence,
      HeartbeatEvidenceKind::Reachable { latency_ms: now_ms.saturating_sub(pending.sent_at_ms) },
    ))
  }

  /// Collects timeout evidence for pending requests whose deadline has passed.
  pub fn collect_timeouts(&mut self, now_ms: u64) -> Vec<HeartbeatEvidence> {
    let expired = self
      .pending
      .iter()
      .filter(|(_, pending)| pending.deadline_ms < now_ms)
      .map(|(key, _)| key.clone())
      .collect::<Vec<_>>();
    let mut evidence = Vec::new();
    for key in expired {
      if self.pending.remove(&key).is_none() {
        continue;
      }
      let kind = if self.has_success.get(&key.peer).copied().unwrap_or(false)
        || self.first_miss_reported.get(&key.peer).copied().unwrap_or(false)
      {
        HeartbeatEvidenceKind::Missed
      } else {
        self.first_miss_reported.insert(key.peer.clone(), true);
        HeartbeatEvidenceKind::FirstMissed
      };
      evidence.push(HeartbeatEvidence::new(self.local.clone(), key.peer, key.sequence, kind));
    }
    evidence
  }

  /// Removes all heartbeat state associated with a peer that left the target set.
  pub fn remove_peer(&mut self, peer: &UniqueAddress) {
    self.next_sequences.remove(peer);
    self.has_success.remove(peer);
    self.first_miss_reported.remove(peer);
    self.pending.retain(|key, _| &key.peer != peer);
  }

  fn next_sequence(&mut self, peer: UniqueAddress) -> u64 {
    let entry = self.next_sequences.entry(peer).or_insert(0);
    *entry += 1;
    *entry
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PendingHeartbeatKey {
  peer:     UniqueAddress,
  sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingHeartbeat {
  sent_at_ms:  u64,
  deadline_ms: u64,
}
