use alloc::collections::VecDeque;

use super::{
  DeterministicEvent, SchedulerDiagnosticsEvent, deterministic_log::DeterministicLog,
  deterministic_replay::DeterministicReplay, diagnostics_registry::DiagnosticsRegistry,
  scheduler_diagnostics_subscription::SchedulerDiagnosticsSubscription,
};

const DEFAULT_STREAM_CAPACITY: usize = 256;

/// Aggregates scheduler diagnostics state.
pub struct SchedulerDiagnostics {
  deterministic_log:  Option<DeterministicLog>,
  registry:           DiagnosticsRegistry,
  next_subscriber_id: u64,
  stream_buffer:      VecDeque<SchedulerDiagnosticsEvent>,
  stream_capacity:    usize,
}

impl SchedulerDiagnostics {
  /// Creates a diagnostics container with logging disabled.
  #[must_use]
  pub fn new() -> Self {
    Self::with_capacity(DEFAULT_STREAM_CAPACITY)
  }

  /// Creates a diagnostics container with the provided stream capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    let bounded = capacity.max(1);
    Self {
      deterministic_log:  None,
      registry:           DiagnosticsRegistry::new(),
      next_subscriber_id: 1,
      stream_buffer:      VecDeque::new(),
      stream_capacity:    bounded,
    }
  }

  /// Enables deterministic logging with the requested capacity.
  pub fn enable_deterministic_log(&mut self, capacity: usize) {
    self.deterministic_log = Some(DeterministicLog::with_capacity(capacity));
  }

  /// Returns whether deterministic logging is enabled.
  #[must_use]
  pub const fn is_log_enabled(&self) -> bool {
    self.deterministic_log.is_some()
  }

  /// Returns the current log entries.
  #[must_use]
  pub fn deterministic_log(&self) -> &[DeterministicEvent] {
    self.deterministic_log.as_ref().map_or(&[], |log| log.entries())
  }

  /// Returns an iterator that can replay deterministic events in order.
  #[must_use]
  pub fn replay(&self) -> DeterministicReplay<'_> {
    DeterministicReplay::new(self.deterministic_log())
  }

  /// Registers a diagnostics subscriber with the requested queue capacity.
  pub fn subscribe(&mut self, capacity: usize) -> SchedulerDiagnosticsSubscription {
    let id = self.alloc_subscriber_id();
    let buffer = self.registry.register(id, capacity.max(1));
    if !self.stream_buffer.is_empty() {
      for event in self.stream_buffer.iter() {
        buffer.push(event);
      }
      self.stream_buffer.clear();
    }
    SchedulerDiagnosticsSubscription::new(id, self.registry.clone(), buffer)
  }

  /// Publishes a stream event to subscribers, returning whether any queue dropped data.
  pub fn publish_stream_event(&mut self, event: SchedulerDiagnosticsEvent) -> bool {
    let outcome = self.registry.publish(&event);
    if !outcome.delivered {
      self.enqueue_buffered_event(event);
      return false;
    }
    outcome.dropped
  }

  pub(crate) fn record(&mut self, event: DeterministicEvent) {
    if let Some(log) = &mut self.deterministic_log {
      log.record(event);
    }
  }

  const fn alloc_subscriber_id(&mut self) -> u64 {
    let id = self.next_subscriber_id;
    self.next_subscriber_id = self.next_subscriber_id.wrapping_add(1);
    if self.next_subscriber_id == 0 {
      self.next_subscriber_id = 1;
    }
    id
  }

  fn enqueue_buffered_event(&mut self, event: SchedulerDiagnosticsEvent) {
    if self.stream_buffer.len() >= self.stream_capacity {
      self.stream_buffer.pop_front();
    }
    self.stream_buffer.push_back(event);
  }
}

impl Default for SchedulerDiagnostics {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for SchedulerDiagnostics {
  fn clone(&self) -> Self {
    Self {
      deterministic_log:  self.deterministic_log.clone(),
      registry:           self.registry.clone(),
      next_subscriber_id: self.next_subscriber_id,
      stream_buffer:      self.stream_buffer.clone(),
      stream_capacity:    self.stream_capacity,
    }
  }
}
