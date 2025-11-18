//! Records remoting metrics and diagnostic samples.

use alloc::{
  collections::{BTreeMap, VecDeque},
  string::{String, ToString},
  vec::Vec,
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  flight_recorder::{correlation_trace::CorrelationTrace, remoting_metric::RemotingMetric},
  remoting_connection_snapshot::RemotingConnectionSnapshot,
};

#[cfg(test)]
mod tests;

/// Records remoting observability events.
#[derive(Clone)]
pub struct RemotingFlightRecorder {
  inner: ArcShared<RecorderInner>,
}

struct RecorderInner {
  #[allow(unused)]
  capacity:          usize,
  suspect_counts:    ToolboxMutex<BTreeMap<String, usize>, NoStdToolbox>,
  reachable_counts:  ToolboxMutex<BTreeMap<String, usize>, NoStdToolbox>,
  metrics:           ToolboxMutex<VecDeque<RemotingMetric>, NoStdToolbox>,
  traces:            ToolboxMutex<VecDeque<CorrelationTrace>, NoStdToolbox>,
  endpoint_snapshot: ToolboxMutex<Vec<RemotingConnectionSnapshot>, NoStdToolbox>,
}

impl RemotingFlightRecorder {
  /// Creates a new recorder with the specified ring buffer capacity.
  #[must_use]
  pub fn new(capacity: usize) -> Self {
    let inner = RecorderInner {
      capacity,
      suspect_counts: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()),
      reachable_counts: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()),
      metrics: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(VecDeque::new()),
      traces: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(VecDeque::new()),
      endpoint_snapshot: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()),
    };
    Self { inner: ArcShared::new(inner) }
  }

  /// Records a suspect event for the authority.
  pub fn record_suspect(&self, authority: &str) {
    let mut counts = self.inner.suspect_counts.lock();
    *counts.entry(authority.to_string()).or_insert(0) += 1;
  }

  /// Records that an authority became reachable again.
  pub fn record_reachable(&self, authority: &str) {
    let mut counts = self.inner.reachable_counts.lock();
    *counts.entry(authority.to_string()).or_insert(0) += 1;
  }

  /// Returns number of suspect events recorded for the authority (test helper).
  #[must_use]
  pub fn suspect_events(&self, authority: &str) -> usize {
    self.inner.suspect_counts.lock().get(authority).copied().unwrap_or(0)
  }

  /// Returns number of reachable events recorded for the authority (test helper).
  #[must_use]
  pub fn reachable_events(&self, authority: &str) -> usize {
    self.inner.reachable_counts.lock().get(authority).copied().unwrap_or(0)
  }

  /// Records a metric sample.
  pub fn record_metric(&self, metric: RemotingMetric) {
    let mut buffer = self.inner.metrics.lock();
    push_with_capacity(&mut buffer, self.inner.capacity, metric);
  }

  /// Returns snapshot of recorded metrics with the oldest sample first.
  #[must_use]
  pub fn metrics_snapshot(&self) -> Vec<RemotingMetric> {
    self.inner.metrics.lock().iter().cloned().collect()
  }

  /// Records a correlation trace entry.
  pub fn record_trace(&self, trace: CorrelationTrace) {
    let mut buffer = self.inner.traces.lock();
    push_with_capacity(&mut buffer, self.inner.capacity, trace);
  }

  /// Returns recorded correlation traces.
  #[must_use]
  pub fn traces_snapshot(&self) -> Vec<CorrelationTrace> {
    self.inner.traces.lock().iter().cloned().collect()
  }

  /// Updates the cached endpoint health snapshot.
  pub fn update_endpoint_snapshot(&self, snapshot: Vec<RemotingConnectionSnapshot>) {
    *self.inner.endpoint_snapshot.lock() = snapshot;
  }

  /// Returns the latest endpoint health snapshot.
  #[must_use]
  pub fn endpoint_snapshot(&self) -> Vec<RemotingConnectionSnapshot> {
    self.inner.endpoint_snapshot.lock().clone()
  }
}

fn push_with_capacity<T>(buffer: &mut VecDeque<T>, capacity: usize, value: T) {
  if capacity == 0 {
    return;
  }
  if buffer.len() == capacity {
    buffer.pop_front();
  }
  buffer.push_back(value);
}
