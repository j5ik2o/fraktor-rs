//! Records remoting metrics for later inspection.

#[cfg(test)]
mod tests;

use alloc::{collections::VecDeque, string::String};

use fraktor_actor_rs::core::event_stream::{BackpressureSignal, CorrelationId};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};

use super::{
  flight_metric_kind::FlightMetricKind, remoting_flight_recorder_snapshot::RemotingFlightRecorderSnapshot,
  remoting_metric::RemotingMetric,
};

struct FlightBuffer {
  capacity: usize,
  entries:  VecDeque<RemotingMetric>,
}

impl FlightBuffer {
  fn new(capacity: usize) -> Self {
    Self { capacity, entries: VecDeque::with_capacity(capacity) }
  }

  fn push(&mut self, record: RemotingMetric) {
    if self.entries.len() == self.capacity {
      self.entries.pop_front();
    }
    self.entries.push_back(record);
  }
}

/// Ring-buffer backed recorder storing recent observability metrics.
pub struct RemotingFlightRecorder {
  buffer: ArcShared<NoStdMutex<FlightBuffer>>,
}

impl RemotingFlightRecorder {
  /// Creates a new recorder with the provided capacity.
  #[must_use]
  pub fn new(capacity: usize) -> Self {
    Self { buffer: ArcShared::new(NoStdMutex::new(FlightBuffer::new(capacity.max(1)))) }
  }

  /// Records a backpressure signal emitted by transports.
  pub fn record_backpressure(
    &self,
    authority: impl Into<String>,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
    timestamp_ms: u64,
  ) {
    self.push(RemotingMetric::new(authority, FlightMetricKind::Backpressure(signal), correlation_id, timestamp_ms));
  }

  /// Records a suspect notification emitted by the failure detector.
  pub fn record_suspect(
    &self,
    authority: impl Into<String>,
    phi: f64,
    correlation_id: CorrelationId,
    timestamp_ms: u64,
  ) {
    self.push(RemotingMetric::new(authority, FlightMetricKind::Suspect { phi }, correlation_id, timestamp_ms));
  }

  /// Records a reachable signal following a suspect period.
  pub fn record_reachable(&self, authority: impl Into<String>, correlation_id: CorrelationId, timestamp_ms: u64) {
    self.push(RemotingMetric::new(authority, FlightMetricKind::Reachable, correlation_id, timestamp_ms));
  }

  fn push(&self, record: RemotingMetric) {
    self.buffer.lock().push(record);
  }

  /// Returns a snapshot of buffered metrics.
  #[must_use]
  pub fn snapshot(&self) -> RemotingFlightRecorderSnapshot {
    let records = self.buffer.lock().entries.iter().cloned().collect();
    RemotingFlightRecorderSnapshot { records }
  }
}
