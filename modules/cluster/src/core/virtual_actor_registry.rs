//! Manages virtual actor activations and passivation.

use alloc::{collections::BTreeMap, format, string::{String, ToString}, vec::Vec};

use crate::core::{
  activation_error::ActivationError,
  activation_record::ActivationRecord,
  grain_key::GrainKey,
  pid_cache::PidCache,
  rendezvous_hasher::RendezvousHasher,
  virtual_actor_event::VirtualActorEvent,
};

struct ActivationEntry {
  record: ActivationRecord,
  authority: String,
  last_seen: u64,
}

/// Registry that keeps track of active grains.
pub struct VirtualActorRegistry {
  activations: BTreeMap<GrainKey, ActivationEntry>,
  pid_cache: PidCache,
  pid_ttl_secs: u64,
  events: Vec<VirtualActorEvent>,
}

impl VirtualActorRegistry {
  /// Creates a new registry.
  pub fn new(cache_capacity: usize, pid_ttl_secs: u64) -> Self {
    Self { activations: BTreeMap::new(), pid_cache: PidCache::new(cache_capacity), pid_ttl_secs, events: Vec::new() }
  }

  /// Ensures an activation exists and returns its PID.
  pub fn ensure_activation(
    &mut self,
    key: GrainKey,
    authorities: &[String],
    now: u64,
    snapshot_required: bool,
    snapshot: Option<Vec<u8>>,
  ) -> Result<String, ActivationError> {
    let Some(owner) = RendezvousHasher::select(authorities, &key) else {
      return Err(ActivationError::NoAuthority);
    };

    if snapshot_required && snapshot.is_none() {
      self.events.push(VirtualActorEvent::SnapshotMissing { key: key.clone() });
      return Err(ActivationError::SnapshotMissing { key: key.value().to_string() });
    }

    if let Some(entry) = self.activations.get_mut(&key) {
      if entry.authority == *owner {
        entry.last_seen = now;
        self.events.push(VirtualActorEvent::Hit { key: key.clone(), pid: entry.record.pid.clone() });
        self.pid_cache.put(key.clone(), entry.record.pid.clone(), owner.clone(), now, self.pid_ttl_secs);
        return Ok(entry.record.pid.clone());
      }
    }

    let pid = format!("{}::{}", owner, key.value());
    let record = ActivationRecord::new(pid.clone(), snapshot, 0);
    let entry = ActivationEntry { record: record.clone(), authority: owner.clone(), last_seen: now };
    let replaced = self.activations.insert(key.clone(), entry);
    self.pid_cache.put(key.clone(), pid.clone(), owner.clone(), now, self.pid_ttl_secs);

    if replaced.is_some() {
      self.events.push(VirtualActorEvent::Reactivated { key: key.clone(), pid: pid.clone(), authority: owner.clone() });
    } else {
      self.events.push(VirtualActorEvent::Activated { key: key.clone(), pid: pid.clone(), authority: owner.clone() });
    }

    Ok(pid)
  }

  /// Returns PID from cache if present and not expired.
  pub fn cached_pid(&mut self, key: &GrainKey, now: u64) -> Option<String> {
    self.pid_cache.get(key, now)
  }

  /// Invalidates all activations and cache entries for an authority (e.g., quarantine).
  pub fn invalidate_authority(&mut self, authority: &str) {
    self.pid_cache.invalidate_authority(authority);
    let to_drop: Vec<_> = self
      .activations
      .iter()
      .filter(|(_, entry)| entry.authority == authority)
      .map(|(key, _)| key.clone())
      .collect();
    for key in to_drop {
      self.activations.remove(&key);
      self.events.push(VirtualActorEvent::Passivated { key });
    }
  }

  /// Drops activations whose authorities disappeared.
  pub fn invalidate_absent_authorities(&mut self, active_authorities: &[String]) {
    self.pid_cache.invalidate_absent_authorities(active_authorities);
    let to_drop: Vec<_> = self
      .activations
      .iter()
      .filter(|(_, entry)| !active_authorities.contains(&entry.authority))
      .map(|(key, _)| key.clone())
      .collect();
    for key in to_drop {
      self.activations.remove(&key);
      self.events.push(VirtualActorEvent::Passivated { key });
    }
  }

  /// Passivates idle activations.
  pub fn passivate_idle(&mut self, now: u64, idle_ttl: u64) {
    let to_passivate: Vec<_> = self
      .activations
      .iter()
      .filter(|(_, entry)| now.saturating_sub(entry.last_seen) >= idle_ttl)
      .map(|(key, _)| key.clone())
      .collect();

    for key in to_passivate {
      self.activations.remove(&key);
      self.pid_cache.invalidate_key(&key);
      self.events.push(VirtualActorEvent::Passivated { key });
    }
  }

  /// Drains events.
  pub fn drain_events(&mut self) -> Vec<VirtualActorEvent> {
    core::mem::take(&mut self.events)
  }
}

#[cfg(test)]
mod tests;
