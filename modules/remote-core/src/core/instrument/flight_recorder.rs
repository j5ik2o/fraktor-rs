//! In-memory ring-buffer flight recorder.

use alloc::{collections::VecDeque, string::String};

use fraktor_actor_core_rs::core::kernel::event::stream::CorrelationId;

use crate::core::{
  instrument::{
    flight_recorder_event::FlightRecorderEvent, flight_recorder_snapshot::RemotingFlightRecorderSnapshot,
    handshake_phase::HandshakePhase,
  },
  transport::BackpressureSignal,
};

/// Bounded ring buffer of [`FlightRecorderEvent`]s used for observability.
///
/// Built on `alloc::collections::VecDeque` per design Decision 12 — `heapless`
/// is deliberately avoided because the core can depend on `alloc`. Once the
/// capacity is reached, recording a new event evicts the oldest one.
#[derive(Clone, Debug)]
pub struct RemotingFlightRecorder {
  capacity: usize,
  events:   VecDeque<FlightRecorderEvent>,
}

impl RemotingFlightRecorder {
  /// Creates a new recorder with the given capacity.
  ///
  /// A capacity of `0` disables recording entirely (every `record_*` call
  /// becomes a no-op).
  #[must_use]
  pub fn new(capacity: usize) -> Self {
    Self { capacity, events: VecDeque::with_capacity(capacity) }
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the number of events currently stored (never exceeds capacity).
  #[must_use]
  pub fn len(&self) -> usize {
    self.events.len()
  }

  /// Returns `true` when the recorder currently holds no events.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.events.is_empty()
  }

  /// Inserts `event`, evicting the oldest entry when capacity is reached.
  pub fn record(&mut self, event: FlightRecorderEvent) {
    if self.capacity == 0 {
      return;
    }
    if self.events.len() == self.capacity {
      self.events.pop_front();
    }
    self.events.push_back(event);
  }

  /// Records a `Send` event at `now_ms` (monotonic millis).
  pub fn record_send(
    &mut self,
    authority: impl Into<String>,
    correlation_id: CorrelationId,
    priority: u8,
    size: u32,
    now_ms: u64,
  ) {
    self.record(FlightRecorderEvent::Send { authority: authority.into(), correlation_id, priority, size, now_ms });
  }

  /// Records a `Receive` event at `now_ms` (monotonic millis).
  pub fn record_receive(
    &mut self,
    authority: impl Into<String>,
    correlation_id: CorrelationId,
    size: u32,
    now_ms: u64,
  ) {
    self.record(FlightRecorderEvent::Receive { authority: authority.into(), correlation_id, size, now_ms });
  }

  /// Records a `Handshake` event at `now_ms` (monotonic millis).
  pub fn record_handshake(&mut self, authority: impl Into<String>, phase: HandshakePhase, now_ms: u64) {
    self.record(FlightRecorderEvent::Handshake { authority: authority.into(), phase, now_ms });
  }

  /// Records a `Quarantine` event at `now_ms` (monotonic millis).
  pub fn record_quarantine(&mut self, authority: impl Into<String>, reason: impl Into<String>, now_ms: u64) {
    self.record(FlightRecorderEvent::Quarantine { authority: authority.into(), reason: reason.into(), now_ms });
  }

  /// Records a `Backpressure` event at `now_ms` (monotonic millis).
  pub fn record_backpressure(
    &mut self,
    authority: impl Into<String>,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
    now_ms: u64,
  ) {
    self.record(FlightRecorderEvent::Backpressure { authority: authority.into(), signal, correlation_id, now_ms });
  }

  /// Returns an immutable [`RemotingFlightRecorderSnapshot`] of the current
  /// event buffer (oldest first).
  #[must_use]
  pub fn snapshot(&self) -> RemotingFlightRecorderSnapshot {
    RemotingFlightRecorderSnapshot::new(self.events.iter().cloned().collect())
  }
}
