//! Versioned membership table and state machine.

use alloc::{collections::BTreeMap, string::{String, ToString}, vec, vec::Vec};

use crate::core::{
  membership_delta::MembershipDelta,
  membership_error::MembershipError,
  membership_event::MembershipEvent,
  membership_snapshot::MembershipSnapshot,
  membership_version::MembershipVersion,
  node_record::NodeRecord,
  node_status::NodeStatus,
};

#[cfg(test)]
mod tests;

/// Holds membership records and emits versioned deltas.
#[derive(Debug)]
pub struct MembershipTable {
  version: MembershipVersion,
  entries: BTreeMap<String, NodeRecord>,
  heartbeat_miss_counters: BTreeMap<String, u32>,
  max_heartbeat_misses: u32,
  events: Vec<MembershipEvent>,
}

impl MembershipTable {
  /// Creates an empty membership table.
  pub fn new(max_heartbeat_misses: u32) -> Self {
    Self {
      version: MembershipVersion::zero(),
      entries: BTreeMap::new(),
      heartbeat_miss_counters: BTreeMap::new(),
      max_heartbeat_misses,
      events: Vec::new(),
    }
  }

  /// Attempts to join the cluster with the given node and authority.
  pub fn try_join(&mut self, node_id: String, authority: String) -> Result<MembershipDelta, MembershipError> {
    if let Some(existing) = self.entries.get(&authority) {
      if existing.node_id != node_id {
        self.events.push(MembershipEvent::AuthorityConflict {
          authority: authority.clone(),
          existing_node_id: existing.node_id.clone(),
          requested_node_id: node_id.clone(),
        });

        return Err(MembershipError::AuthorityConflict {
          authority,
          existing_node_id: existing.node_id.clone(),
          requested_node_id: node_id,
        });
      }

      return Ok(MembershipDelta::new(self.version, self.version, vec![existing.clone()]));
    }

    let from = self.version;
    self.version = self.version.next();

    let record = NodeRecord::new(node_id.clone(), authority.clone(), NodeStatus::Up, self.version);
    self.entries.insert(authority.clone(), record.clone());
    self.heartbeat_miss_counters.insert(authority.clone(), 0);

    self.events.push(MembershipEvent::Joined { node_id, authority });

    Ok(MembershipDelta::new(from, self.version, vec![record]))
  }

  /// Marks the authority as leaving and then removed.
  pub fn mark_left(&mut self, authority: &str) -> Result<MembershipDelta, MembershipError> {
    let Some(record) = self.entries.get_mut(authority) else {
      return Err(MembershipError::UnknownAuthority { authority: authority.to_string() });
    };

    let from = self.version;
    self.version = self.version.next();

    record.status = NodeStatus::Removed;
    record.version = self.version;

    self.events.push(MembershipEvent::Left { node_id: record.node_id.clone(), authority: record.authority.clone() });

    Ok(MembershipDelta::new(from, self.version, vec![record.clone()]))
  }

  /// Increments heartbeat misses; returns a delta when it becomes unreachable.
  pub fn mark_heartbeat_miss(&mut self, authority: &str) -> Option<MembershipDelta> {
    let Some(record) = self.entries.get_mut(authority) else {
      return None;
    };

    if matches!(record.status, NodeStatus::Removed | NodeStatus::Unreachable) {
      return None;
    }

    let counter = self.heartbeat_miss_counters.entry(authority.to_string()).or_insert(0);
    *counter += 1;

    if *counter < self.max_heartbeat_misses {
      return None;
    }

    let from = self.version;
    self.version = self.version.next();

    record.status = NodeStatus::Unreachable;
    record.version = self.version;

    self.events.push(MembershipEvent::MarkedUnreachable {
      node_id: record.node_id.clone(),
      authority: record.authority.clone(),
    });

    Some(MembershipDelta::new(from, self.version, vec![record.clone()]))
  }

  /// Applies a received membership delta.
  pub fn apply_delta(&mut self, delta: MembershipDelta) {
    if delta.to <= self.version {
      return;
    }

    self.version = delta.to;

    for record in delta.entries {
      self.heartbeat_miss_counters.insert(record.authority.clone(), 0);
      self.entries.insert(record.authority.clone(), record);
    }
  }

  /// Returns a snapshot for handshake.
  pub fn snapshot(&self) -> MembershipSnapshot {
    MembershipSnapshot::new(self.version, self.entries.values().cloned().collect())
  }

  /// Returns current version.
  pub const fn version(&self) -> MembershipVersion {
    self.version
  }

  /// Gets a record by authority.
  pub fn record(&self, authority: &str) -> Option<&NodeRecord> {
    self.entries.get(authority)
  }

  /// Drains buffered events.
  pub fn drain_events(&mut self) -> Vec<MembershipEvent> {
    core::mem::take(&mut self.events)
  }
}
