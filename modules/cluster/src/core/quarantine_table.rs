//! Quarantine table with TTL management.

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::time::TimerInstant;

use crate::core::{quarantine_entry::QuarantineEntry, quarantine_event::QuarantineEvent};

/// Stores quarantined authorities with expiration.
pub struct QuarantineTable {
  entries: BTreeMap<String, QuarantineEntry>,
}

impl QuarantineTable {
  /// Creates an empty quarantine table.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Returns true if the authority is quarantined.
  #[must_use]
  pub fn contains(&self, authority: &str) -> bool {
    self.entries.contains_key(authority)
  }

  /// Returns a snapshot of quarantined entries.
  #[must_use]
  pub fn snapshot(&self) -> Vec<QuarantineEntry> {
    self.entries.values().cloned().collect()
  }

  /// Adds or updates a quarantine entry.
  pub fn quarantine(&mut self, authority: String, reason: String, now: TimerInstant, ttl: Duration) -> QuarantineEvent {
    let expires_at = add_duration(now, ttl);
    let entry = QuarantineEntry::new(authority.clone(), reason.clone(), expires_at);
    self.entries.insert(authority.clone(), entry);
    QuarantineEvent::Quarantined { authority, reason }
  }

  /// Clears a quarantine entry.
  pub fn clear(&mut self, authority: &str) -> Option<QuarantineEvent> {
    self.entries.remove(authority).map(|entry| QuarantineEvent::Cleared { authority: entry.authority })
  }

  /// Clears expired entries and returns emitted events.
  pub fn poll_expired(&mut self, now: TimerInstant) -> Vec<QuarantineEvent> {
    let mut cleared = Vec::new();
    let expired = self
      .entries
      .iter()
      .filter(|(_, entry)| entry.expires_at <= now)
      .map(|(authority, _)| authority.clone())
      .collect::<Vec<_>>();
    for authority in expired {
      if let Some(event) = self.clear(&authority) {
        cleared.push(event);
      }
    }
    cleared
  }
}

impl Default for QuarantineTable {
  fn default() -> Self {
    Self::new()
  }
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
