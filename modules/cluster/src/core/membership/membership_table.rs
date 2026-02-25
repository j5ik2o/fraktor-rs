//! Versioned membership table and state machine.

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec,
  vec::Vec,
};

use super::{
  MembershipDelta, MembershipError, MembershipEvent, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus,
};

#[cfg(test)]
mod tests;

/// Holds membership records and emits versioned deltas.
#[derive(Debug)]
pub struct MembershipTable {
  version:                 MembershipVersion,
  entries:                 BTreeMap<String, NodeRecord>,
  heartbeat_miss_counters: BTreeMap<String, u32>,
  max_heartbeat_misses:    u32,
  events:                  Vec<MembershipEvent>,
}

impl MembershipTable {
  /// Creates an empty membership table.
  #[must_use]
  pub const fn new(max_heartbeat_misses: u32) -> Self {
    Self {
      version: MembershipVersion::zero(),
      entries: BTreeMap::new(),
      heartbeat_miss_counters: BTreeMap::new(),
      max_heartbeat_misses,
      events: Vec::new(),
    }
  }

  /// Attempts to join the cluster with the given node and authority.
  ///
  /// # Errors
  ///
  /// Returns `MembershipError::AuthorityConflict` if the authority is already registered with a
  /// different node ID.
  pub fn try_join(
    &mut self,
    node_id: String,
    authority: String,
    app_version: String,
    roles: Vec<String>,
  ) -> Result<MembershipDelta, MembershipError> {
    if let Some(existing) = self.entries.get_mut(&authority) {
      if existing.node_id != node_id {
        self.events.push(MembershipEvent::AuthorityConflict {
          authority:         authority.clone(),
          existing_node_id:  existing.node_id.clone(),
          requested_node_id: node_id.clone(),
        });

        return Err(MembershipError::AuthorityConflict {
          authority,
          existing_node_id: existing.node_id.clone(),
          requested_node_id: node_id,
        });
      }

      if matches!(existing.status, NodeStatus::Removed | NodeStatus::Dead) {
        let from = self.version;
        self.version = self.version.next();
        existing.status = NodeStatus::Joining;
        existing.version = self.version;
        existing.join_version = self.version;
        existing.app_version = app_version;
        existing.roles = roles;
        return Ok(MembershipDelta::new(from, self.version, vec![existing.clone()]));
      }

      return Ok(MembershipDelta::new(self.version, self.version, vec![existing.clone()]));
    }

    let from = self.version;
    self.version = self.version.next();

    let record =
      NodeRecord::new(node_id.clone(), authority.clone(), NodeStatus::Joining, self.version, app_version, roles);
    self.entries.insert(authority.clone(), record.clone());
    self.heartbeat_miss_counters.insert(authority.clone(), 0);

    self.events.push(MembershipEvent::Joined { node_id, authority });

    Ok(MembershipDelta::new(from, self.version, vec![record]))
  }

  /// Marks the authority as leaving (`Exiting`) and then removed.
  ///
  /// # Errors
  ///
  /// Returns `MembershipError::UnknownAuthority` if the authority is not found in the table.
  pub fn mark_left(&mut self, authority: &str) -> Result<MembershipDelta, MembershipError> {
    let Some(record) = self.entries.get_mut(authority) else {
      return Err(MembershipError::UnknownAuthority { authority: authority.to_string() });
    };

    if matches!(record.status, NodeStatus::Removed | NodeStatus::Dead) {
      return Err(MembershipError::InvalidTransition {
        authority: authority.to_string(),
        from:      record.status,
        to:        NodeStatus::Exiting,
      });
    }

    let from = self.version;
    self.version = self.version.next();
    if record.status == NodeStatus::Exiting {
      record.status = NodeStatus::Removed;
      self
        .events
        .push(MembershipEvent::Left { node_id: record.node_id.clone(), authority: record.authority.clone() });
    } else {
      record.status = NodeStatus::Exiting;
    }
    record.version = self.version;

    Ok(MembershipDelta::new(from, self.version, vec![record.clone()]))
  }

  /// Increments heartbeat misses; returns a delta when it becomes suspect.
  pub fn mark_heartbeat_miss(&mut self, authority: &str) -> Option<MembershipDelta> {
    let record = self.entries.get_mut(authority)?;

    if !record.status.is_active() {
      return None;
    }

    let counter = self.heartbeat_miss_counters.entry(authority.to_string()).or_insert(0);
    *counter += 1;

    if *counter < self.max_heartbeat_misses {
      return None;
    }

    let from = self.version;
    self.version = self.version.next();

    record.status = NodeStatus::Suspect;
    record.version = self.version;

    self
      .events
      .push(MembershipEvent::MarkedSuspect { node_id: record.node_id.clone(), authority: record.authority.clone() });

    Some(MembershipDelta::new(from, self.version, vec![record.clone()]))
  }

  /// Marks the authority as reachable (Up) if currently Joining or Suspect.
  ///
  /// # Errors
  ///
  /// Returns `MembershipError::UnknownAuthority` if the authority is not found.
  pub fn mark_up(&mut self, authority: &str) -> Result<Option<MembershipDelta>, MembershipError> {
    let Some(record) = self.entries.get_mut(authority) else {
      return Err(MembershipError::UnknownAuthority { authority: authority.to_string() });
    };

    match record.status {
      | NodeStatus::Up => return Ok(None),
      | NodeStatus::Joining | NodeStatus::Suspect => {},
      | _ => {
        return Err(MembershipError::InvalidTransition {
          authority: authority.to_string(),
          from:      record.status,
          to:        NodeStatus::Up,
        });
      },
    }

    let from = self.version;
    self.version = self.version.next();
    record.status = NodeStatus::Up;
    record.version = self.version;

    Ok(Some(MembershipDelta::new(from, self.version, vec![record.clone()])))
  }

  /// Marks the authority as suspect.
  ///
  /// # Errors
  ///
  /// Returns `MembershipError::UnknownAuthority` if the authority is not found.
  pub fn mark_suspect(&mut self, authority: &str) -> Result<Option<MembershipDelta>, MembershipError> {
    let Some(record) = self.entries.get_mut(authority) else {
      return Err(MembershipError::UnknownAuthority { authority: authority.to_string() });
    };

    match record.status {
      | NodeStatus::Suspect => return Ok(None),
      | NodeStatus::Up | NodeStatus::Joining => {},
      | _ => {
        return Err(MembershipError::InvalidTransition {
          authority: authority.to_string(),
          from:      record.status,
          to:        NodeStatus::Suspect,
        });
      },
    }

    let from = self.version;
    self.version = self.version.next();
    record.status = NodeStatus::Suspect;
    record.version = self.version;

    Ok(Some(MembershipDelta::new(from, self.version, vec![record.clone()])))
  }

  /// Marks the authority as dead.
  ///
  /// # Errors
  ///
  /// Returns `MembershipError::UnknownAuthority` if the authority is not found.
  pub fn mark_dead(&mut self, authority: &str) -> Result<Option<MembershipDelta>, MembershipError> {
    let Some(record) = self.entries.get_mut(authority) else {
      return Err(MembershipError::UnknownAuthority { authority: authority.to_string() });
    };

    match record.status {
      | NodeStatus::Dead => return Ok(None),
      | NodeStatus::Suspect => {},
      | _ => {
        return Err(MembershipError::InvalidTransition {
          authority: authority.to_string(),
          from:      record.status,
          to:        NodeStatus::Dead,
        });
      },
    }

    let from = self.version;
    self.version = self.version.next();
    record.status = NodeStatus::Dead;
    record.version = self.version;

    Ok(Some(MembershipDelta::new(from, self.version, vec![record.clone()])))
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
  #[must_use]
  pub fn snapshot(&self) -> MembershipSnapshot {
    MembershipSnapshot::new(self.version, self.entries.values().cloned().collect())
  }

  /// Returns current version.
  #[must_use]
  pub const fn version(&self) -> MembershipVersion {
    self.version
  }

  /// Gets a record by authority.
  #[must_use]
  pub fn record(&self, authority: &str) -> Option<&NodeRecord> {
    self.entries.get(authority)
  }

  /// Drains buffered events.
  pub fn drain_events(&mut self) -> Vec<MembershipEvent> {
    core::mem::take(&mut self.events)
  }
}
