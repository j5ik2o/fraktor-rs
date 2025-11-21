//! GrainKey -> PID cache with TTL and authority invalidation.

use alloc::{collections::BTreeMap, format, string::{String, ToString}, vec::Vec};

use crate::core::grain_key::GrainKey;

#[derive(Debug, Clone)]
struct CacheEntry {
  pid: String,
  authority: String,
  expires_at: u64,
}

/// Simple TTL-based PID cache.
pub struct PidCache {
  entries: BTreeMap<GrainKey, CacheEntry>,
  max_entries: usize,
  events: Vec<PidCacheEvent>,
}

/// Events emitted from cache operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidCacheEvent {
  /// Entry was dropped due to TTL or owner change.
  Dropped {
    /// Key that was removed.
    key: GrainKey,
    /// Context of removal.
    reason: String,
  },
}

impl PidCache {
  /// Creates a new cache with the given capacity.
  pub fn new(max_entries: usize) -> Self {
    Self { entries: BTreeMap::new(), max_entries, events: Vec::new() }
  }

  /// Inserts an entry with TTL.
  pub fn put(&mut self, key: GrainKey, pid: String, authority: String, now: u64, ttl_secs: u64) {
    self.evict_if_needed();
    let expires_at = now.saturating_add(ttl_secs);
    self.entries.insert(key, CacheEntry { pid, authority, expires_at });
  }

  /// Fetches a PID if not expired.
  pub fn get(&mut self, key: &GrainKey, now: u64) -> Option<String> {
    if let Some(entry) = self.entries.get(key)
      && entry.expires_at > now
    {
      return Some(entry.pid.clone());
    }

    if let Some(entry) = self.entries.remove(key) {
      self.events.push(PidCacheEvent::Dropped { key: key.clone(), reason: format!("expired_at_{}", entry.expires_at) });
    }
    None
  }

  /// Invalidates entries owned by a quarantined authority.
  pub fn invalidate_authority(&mut self, authority: &str) {
    let to_drop: Vec<_> = self
      .entries
      .iter()
      .filter(|(_, entry)| entry.authority == authority)
      .map(|(key, _)| key.clone())
      .collect();
    for key in to_drop {
      self.entries.remove(&key);
      self.events.push(PidCacheEvent::Dropped { key, reason: "quarantine".to_string() });
    }
  }

  /// Invalidates a single key.
  pub fn invalidate_key(&mut self, key: &GrainKey) {
    if self.entries.remove(key).is_some() {
      self.events.push(PidCacheEvent::Dropped { key: key.clone(), reason: "passivated".to_string() });
    }
  }

  /// Invalidates entries whose authority is no longer present.
  pub fn invalidate_absent_authorities(&mut self, active_authorities: &[String]) {
    let to_drop: Vec<_> = self
      .entries
      .iter()
      .filter(|(_, entry)| !active_authorities.contains(&entry.authority))
      .map(|(key, _)| key.clone())
      .collect();
    for key in to_drop {
      self.entries.remove(&key);
      self.events.push(PidCacheEvent::Dropped { key, reason: "missing_authority".to_string() });
    }
  }

  /// Drains emitted events.
  pub fn drain_events(&mut self) -> Vec<PidCacheEvent> {
    core::mem::take(&mut self.events)
  }

  fn evict_if_needed(&mut self) {
    if self.entries.len() < self.max_entries {
      return;
    }

    if let Some(first_key) = self.entries.keys().next().cloned() {
      self.entries.remove(&first_key);
      self.events.push(PidCacheEvent::Dropped { key: first_key, reason: "capacity".to_string() });
    }
  }
}

#[cfg(test)]
mod tests;
