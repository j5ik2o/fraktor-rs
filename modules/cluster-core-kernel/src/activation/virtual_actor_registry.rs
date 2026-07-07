//! Manages virtual actor activations and passivation.

use alloc::{
  collections::BTreeMap,
  format,
  string::{String, ToString},
  vec::Vec,
};

use super::{ActivationError, ActivationRecord, PidCache, PidCacheEvent, RendezvousHasher, VirtualActorEvent};
use crate::{
  extension::{ClusterShardingSettings, PassivationStrategy},
  grain::GrainKey,
};

#[cfg(test)]
#[path = "virtual_actor_registry_passivation_test.rs"]
mod passivation_tests;

#[cfg(test)]
#[path = "virtual_actor_registry_test.rs"]
mod tests;

struct ActivationEntry {
  record:    ActivationRecord,
  authority: String,
  last_seen: u64,
}

/// Registry that keeps track of active grains.
pub struct VirtualActorRegistry {
  activations:  BTreeMap<GrainKey, ActivationEntry>,
  pid_cache:    PidCache,
  pid_ttl_secs: u64,
  events:       Vec<VirtualActorEvent>,
}

impl VirtualActorRegistry {
  /// Creates a new registry.
  #[must_use]
  pub const fn new(cache_capacity: usize, pid_ttl_secs: u64) -> Self {
    Self { activations: BTreeMap::new(), pid_cache: PidCache::new(cache_capacity), pid_ttl_secs, events: Vec::new() }
  }

  /// Returns whether remembered entities are enabled for the given sharding settings.
  #[must_use]
  pub const fn remember_entities_enabled(settings: &ClusterShardingSettings) -> bool {
    settings.remember_entities()
  }

  /// Ensures an activation exists and returns its PID.
  ///
  /// # Errors
  ///
  /// Returns `ActivationError::NoAuthority` if no authorities are provided.
  /// Returns `ActivationError::SnapshotMissing` if a snapshot is required but not provided.
  pub fn ensure_activation(
    &mut self,
    key: &GrainKey,
    authorities: &[String],
    now: u64,
    snapshot_required: bool,
    snapshot: Option<Vec<u8>>,
  ) -> Result<String, ActivationError> {
    let Some(owner) = RendezvousHasher::select(authorities, key) else {
      return Err(ActivationError::NoAuthority);
    };

    if snapshot_required && snapshot.is_none() {
      self.events.push(VirtualActorEvent::SnapshotMissing { key: key.clone() });
      return Err(ActivationError::SnapshotMissing { key: key.value().to_string() });
    }

    if let Some(entry) = self.activations.get_mut(key)
      && entry.authority == *owner
    {
      entry.last_seen = now;
      self.events.push(VirtualActorEvent::Hit { key: key.clone(), pid: entry.record.pid.clone() });
      self.pid_cache.put(key.clone(), entry.record.pid.clone(), owner.clone(), now, self.pid_ttl_secs);
      return Ok(entry.record.pid.clone());
    }

    let pid = format!("{}::{}", owner, key.value());
    let record = ActivationRecord::new(pid.clone(), snapshot, 0);
    let entry = ActivationEntry { record, authority: owner.clone(), last_seen: now };
    let replaced = self.activations.insert(key.clone(), entry);
    self.pid_cache.put(key.clone(), pid.clone(), owner.clone(), now, self.pid_ttl_secs);

    if replaced.is_some() {
      self.events.push(VirtualActorEvent::Reactivated {
        key:       key.clone(),
        pid:       pid.clone(),
        authority: owner.clone(),
      });
    } else {
      self.events.push(VirtualActorEvent::Activated {
        key:       key.clone(),
        pid:       pid.clone(),
        authority: owner.clone(),
      });
    }

    Ok(pid)
  }

  /// Returns PID from cache if present and not expired.
  pub fn cached_pid(&mut self, key: &GrainKey, now: u64) -> Option<String> {
    self.pid_cache.get(key, now)
  }

  /// Returns PID and owner authority from cache if present and not expired.
  pub fn cached_pid_with_authority(&mut self, key: &GrainKey, now: u64) -> Option<(String, String)> {
    self.pid_cache.get_with_authority(key, now)
  }

  /// Invalidates all activations and cache entries for an authority (e.g., quarantine).
  pub fn invalidate_authority(&mut self, authority: &str) {
    self.pid_cache.invalidate_authority(authority);
    let to_drop: Vec<_> =
      self.activations.iter().filter(|(_, entry)| entry.authority == authority).map(|(key, _)| key.clone()).collect();
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

  /// Applies the configured passivation strategy to active entities.
  pub fn passivate_by_strategy(&mut self, strategy: &PassivationStrategy, now: u64) {
    match strategy {
      | PassivationStrategy::Disabled => {},
      | PassivationStrategy::Idle { timeout, .. } => {
        self.passivate_idle(now, timeout.as_secs());
      },
      | PassivationStrategy::ActiveLimit { limit, idle_timeout, .. } => {
        if let Some(idle) = idle_timeout {
          self.passivate_idle(now, idle.as_secs());
        }
        self.passivate_excess_by_last_seen(*limit as usize);
      },
      | PassivationStrategy::Lru { limit, idle_timeout, .. }
      | PassivationStrategy::Mru { limit, idle_timeout, .. }
      | PassivationStrategy::Lfu { limit, idle_timeout, .. } => {
        if let Some(idle) = idle_timeout {
          self.passivate_idle(now, idle.as_secs());
        }
        self.passivate_excess_by_last_seen(*limit as usize);
      },
    }
  }

  fn passivate_excess_by_last_seen(&mut self, limit: usize) {
    if self.activations.len() <= limit {
      return;
    }
    let mut entries: Vec<_> = self.activations.iter().map(|(key, entry)| (key.clone(), entry.last_seen)).collect();
    entries.sort_by_key(|(_, last_seen)| *last_seen);
    let excess = self.activations.len().saturating_sub(limit);
    for (key, _) in entries.into_iter().take(excess) {
      self.remove_activation(&key);
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

  /// Removes an activation and its cache entry for the given key.
  ///
  /// If the key exists, generates a [`VirtualActorEvent::Passivated`] event.
  /// If the key does not exist, this method does nothing.
  pub fn remove_activation(&mut self, key: &GrainKey) {
    // アクティベーションが存在する場合のみ削除処理を実行
    if self.activations.remove(key).is_some() {
      // 対応するキャッシュエントリも削除
      self.pid_cache.invalidate_key(key);
      // Passivated イベントを生成
      self.events.push(VirtualActorEvent::Passivated { key: key.clone() });
    }
  }

  /// Returns active grain keys currently tracked by the registry.
  #[must_use]
  pub fn active_keys(&self) -> Vec<GrainKey> {
    self.activations.keys().cloned().collect()
  }

  /// Drains virtual actor events.
  pub fn drain_events(&mut self) -> Vec<VirtualActorEvent> {
    core::mem::take(&mut self.events)
  }

  /// Drains PID cache events.
  ///
  /// Returns all accumulated cache events and clears the internal buffer.
  pub fn drain_cache_events(&mut self) -> Vec<PidCacheEvent> {
    self.pid_cache.drain_events()
  }

  /// Records an activation entry with explicit authority.
  pub fn record_activation(&mut self, key: &GrainKey, authority: &str, record: &ActivationRecord, now: u64) {
    let pid = record.pid.clone();
    let entry = ActivationEntry { record: record.clone(), authority: authority.to_string(), last_seen: now };
    let replaced = self.activations.insert(key.clone(), entry);
    self.pid_cache.put(key.clone(), pid.clone(), authority.to_string(), now, self.pid_ttl_secs);

    let event = if replaced.is_some() {
      VirtualActorEvent::Reactivated { key: key.clone(), pid, authority: authority.to_string() }
    } else {
      VirtualActorEvent::Activated { key: key.clone(), pid, authority: authority.to_string() }
    };
    self.events.push(event);
  }
}
