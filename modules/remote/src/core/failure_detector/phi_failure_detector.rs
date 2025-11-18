//! Phi accrual failure detector for remote authorities.

#[cfg(test)]
mod tests;

use alloc::{
  collections::{BTreeMap, VecDeque},
  string::{String, ToString},
  vec::Vec,
};

use super::{
  phi_failure_detector_config::PhiFailureDetectorConfig, phi_failure_detector_effect::PhiFailureDetectorEffect,
};

struct PhiEntry {
  last_heartbeat: Option<u64>,
  intervals_ms:   VecDeque<u64>,
  suspect:        bool,
  capacity:       usize,
}

impl PhiEntry {
  fn new(capacity: usize) -> Self {
    Self { last_heartbeat: None, intervals_ms: VecDeque::with_capacity(capacity), suspect: false, capacity }
  }

  fn record_heartbeat(&mut self, now_ms: u64, minimum_interval_ms: u64) {
    if let Some(previous) = self.last_heartbeat {
      let interval = now_ms.saturating_sub(previous).max(minimum_interval_ms);
      if self.intervals_ms.len() == self.capacity {
        self.intervals_ms.pop_front();
      }
      self.intervals_ms.push_back(interval);
    }
    self.last_heartbeat = Some(now_ms);
    self.suspect = false;
  }

  fn phi(&self, now_ms: u64) -> f64 {
    let Some(last) = self.last_heartbeat else {
      return 0.0;
    };
    if self.intervals_ms.is_empty() {
      return 0.0;
    }
    let elapsed = now_ms.saturating_sub(last) as f64;
    let mean = self.intervals_ms.iter().copied().sum::<u64>() as f64 / self.intervals_ms.len() as f64;
    if mean <= 0.0 {
      return 0.0;
    }
    elapsed / mean
  }
}

/// Phi accrual failure detector managing per-authority heartbeats.
pub struct PhiFailureDetector {
  config:  PhiFailureDetectorConfig,
  entries: BTreeMap<String, PhiEntry>,
}

impl PhiFailureDetector {
  /// Creates a detector with the provided configuration.
  #[must_use]
  pub fn new(config: PhiFailureDetectorConfig) -> Self {
    Self { config, entries: BTreeMap::new() }
  }

  fn entry_mut(&mut self, authority: &str) -> &mut PhiEntry {
    self.entries.entry(authority.to_string()).or_insert_with(|| PhiEntry::new(self.config.max_sample_size()))
  }

  /// Records a heartbeat for the authority and returns a reachable effect if one was pending.
  pub fn record_heartbeat(&mut self, authority: &str, now_ms: u64) -> Option<PhiFailureDetectorEffect> {
    let min_interval = self.config.minimum_interval_ms();
    let entry = self.entry_mut(authority);
    let was_suspect = entry.suspect;
    entry.record_heartbeat(now_ms, min_interval);
    if was_suspect { Some(PhiFailureDetectorEffect::Reachable { authority: authority.to_string() }) } else { None }
  }

  /// Polls all authorities and produces suspect events when necessary.
  pub fn poll(&mut self, now_ms: u64) -> Vec<PhiFailureDetectorEffect> {
    self
      .entries
      .iter_mut()
      .filter_map(|(authority, entry)| {
        if entry.last_heartbeat.is_none() || entry.suspect {
          return None;
        }
        let phi = entry.phi(now_ms);
        if phi >= self.config.threshold() {
          entry.suspect = true;
          Some(PhiFailureDetectorEffect::Suspect { authority: authority.clone(), phi })
        } else {
          None
        }
      })
      .collect()
  }
}
