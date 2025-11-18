//! Phi accrual failure detector.

use alloc::{
  collections::{BTreeMap, VecDeque},
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};

use crate::core::flight_recorder::RemotingFlightRecorder;

#[cfg(test)]
mod tests;

use super::{failure_detector_event::FailureDetectorEvent, phi_failure_detector_config::PhiFailureDetectorConfig};

/// Failure detector implementation following the phi accrual algorithm.
pub struct PhiFailureDetector<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  config:   PhiFailureDetectorConfig,
  recorder: RemotingFlightRecorder,
  entries:  ToolboxMutex<BTreeMap<String, Probe>, TB>,
}

impl<TB: RuntimeToolbox + 'static> PhiFailureDetector<TB> {
  /// Creates a new failure detector using the provided configuration.
  #[must_use]
  pub fn new(config: PhiFailureDetectorConfig, recorder: RemotingFlightRecorder) -> Self {
    Self { config, recorder, entries: <TB::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()) }
  }

  /// Records an incoming heartbeat for the authority.
  pub fn record_heartbeat(&self, authority: &str, now: Duration) {
    let mut entries = self.entries.lock();
    let probe = entries.entry(authority.to_string()).or_insert_with(|| Probe::new(self.config.sample_size()));
    probe.record_heartbeat(now, self.config.min_interval());
  }

  /// Evaluates all tracked authorities and returns failure detector events.
  #[must_use]
  pub fn detect(&self, now: Duration) -> Vec<FailureDetectorEvent> {
    let mut entries = self.entries.lock();
    let mut events = Vec::new();
    for (authority, probe) in entries.iter_mut() {
      if probe.try_consume_reachable() {
        events.push(FailureDetectorEvent::Reachable { authority: authority.clone() });
        self.recorder.record_reachable(authority);
        continue;
      }

      let phi = probe.compute_phi(now);
      if phi >= self.config.threshold() && probe.mark_suspect() {
        events.push(FailureDetectorEvent::Suspect { authority: authority.clone(), phi });
        self.recorder.record_suspect(authority);
      } else if phi < self.config.threshold() {
        probe.clear_suspect();
      }
    }
    events
  }
}

struct Probe {
  last_heartbeat:    Option<Duration>,
  samples:           VecDeque<Duration>,
  suspect:           bool,
  reachable_pending: bool,
  capacity:          usize,
}

impl Probe {
  fn new(capacity: usize) -> Self {
    Self {
      last_heartbeat: None,
      samples: VecDeque::with_capacity(capacity),
      suspect: false,
      reachable_pending: false,
      capacity,
    }
  }

  fn record_heartbeat(&mut self, now: Duration, min_interval: Duration) {
    if let Some(last) = self.last_heartbeat
      && let Some(interval) = now.checked_sub(last)
      && interval >= min_interval
    {
      self.samples.push_back(interval);
      if self.samples.len() > self.capacity {
        self.samples.pop_front();
      }
    }

    self.last_heartbeat = Some(now);
    if self.suspect {
      self.suspect = false;
      self.reachable_pending = true;
    }
  }

  fn compute_phi(&self, now: Duration) -> f64 {
    let Some(last) = self.last_heartbeat else {
      return 0.0;
    };
    if self.samples.is_empty() {
      return 0.0;
    }
    let Some(elapsed) = now.checked_sub(last) else {
      return 0.0;
    };
    let mean = self.samples.iter().map(duration_to_millis).sum::<f64>() / self.samples.len() as f64;
    if mean <= f64::EPSILON {
      return 0.0;
    }
    let scaled = duration_to_millis(&elapsed) / mean;
    if scaled <= f64::EPSILON { 0.0 } else { scaled }
  }

  fn mark_suspect(&mut self) -> bool {
    if self.suspect {
      false
    } else {
      self.suspect = true;
      true
    }
  }

  fn clear_suspect(&mut self) {
    self.suspect = false;
  }

  fn try_consume_reachable(&mut self) -> bool {
    if self.reachable_pending {
      self.reachable_pending = false;
      true
    } else {
      false
    }
  }
}

fn duration_to_millis(duration: &Duration) -> f64 {
  duration.as_secs() as f64 * 1_000.0 + f64::from(duration.subsec_nanos()) / 1_000_000.0
}
