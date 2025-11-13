//! Scheduler diagnostics and deterministic logging utilities.

use alloc::{collections::VecDeque, vec::Vec};

use fraktor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use super::{execution_batch::ExecutionBatch, mode::SchedulerMode, warning::SchedulerWarning};

const DEFAULT_STREAM_CAPACITY: usize = 256;

/// Kinds of deterministic log entries emitted by the scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeterministicEvent {
  /// Timer registration event.
  Scheduled {
    /// Identifier of the registered handle.
    handle_id:     u64,
    /// Tick when the registration occurred.
    scheduled_tick: u64,
    /// Deadline tick assigned to the timer.
    deadline_tick:  u64,
  },
  /// Timer execution event.
  Fired {
    /// Identifier of the handle that executed.
    handle_id: u64,
    /// Tick when execution happened.
    fired_tick: u64,
    /// Execution metadata shared with the runnable/message.
    batch:      ExecutionBatch,
  },
  /// Timer cancellation event.
  Cancelled {
    /// Identifier of the cancelled handle.
    handle_id:     u64,
    /// Tick when the cancellation occurred.
    cancelled_tick: u64,
  },
}

/// Aggregates scheduler diagnostics state.
pub struct SchedulerDiagnostics {
  deterministic_log: Option<DeterministicLog>,
  registry:          DiagnosticsRegistry,
  next_subscriber_id: u64,
  stream_buffer:     VecDeque<SchedulerDiagnosticsEvent>,
  stream_capacity:   usize,
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
    Self { deterministic_log: None, registry: DiagnosticsRegistry::new(), next_subscriber_id: 1, stream_buffer: VecDeque::new(), stream_capacity: bounded }
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
    self
      .deterministic_log
      .as_ref()
      .map_or(&[], |log| log.entries.as_slice())
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

  fn alloc_subscriber_id(&mut self) -> u64 {
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
      deterministic_log: self.deterministic_log.clone(),
      registry: self.registry.clone(),
      next_subscriber_id: self.next_subscriber_id,
      stream_buffer: self.stream_buffer.clone(),
      stream_capacity: self.stream_capacity,
    }
  }
}

struct DeterministicLog {
  entries:  Vec<DeterministicEvent>,
  capacity: usize,
}

impl DeterministicLog {
  fn with_capacity(capacity: usize) -> Self {
    Self { entries: Vec::with_capacity(capacity), capacity }
  }

  fn record(&mut self, event: DeterministicEvent) {
    if self.entries.len() < self.capacity {
      self.entries.push(event);
    }
  }
}

impl Clone for DeterministicLog {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), capacity: self.capacity }
  }
}

/// Iterator over recorded deterministic events.
pub struct DeterministicReplay<'a> {
  events:   &'a [DeterministicEvent],
  position: usize,
}

impl<'a> DeterministicReplay<'a> {
  fn new(events: &'a [DeterministicEvent]) -> Self {
    Self { events, position: 0 }
  }

  /// Returns the remaining events without advancing the iterator.
  #[must_use]
  pub const fn as_slice(&self) -> &'a [DeterministicEvent] {
    self.events
  }
}

impl<'a> Iterator for DeterministicReplay<'a> {
  type Item = DeterministicEvent;

  fn next(&mut self) -> Option<Self::Item> {
    if self.position >= self.events.len() {
      return None;
    }
    let event = self.events[self.position];
    self.position += 1;
    Some(event)
  }
}

/// Streams scheduler events to diagnostic subscribers.
#[derive(Clone)]
struct DiagnosticsRegistry {
  entries: ArcShared<NoStdMutex<Vec<DiagnosticsSubscriber>>>,
}

impl DiagnosticsRegistry {
  fn new() -> Self {
    Self { entries: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn register(&self, id: u64, capacity: usize) -> ArcShared<DiagnosticsBuffer> {
    let buffer = ArcShared::new(DiagnosticsBuffer::new(capacity));
    let mut entries = self.entries.lock();
    entries.push(DiagnosticsSubscriber { id, buffer: buffer.clone() });
    buffer
  }

  fn remove(&self, id: u64) {
    let mut entries = self.entries.lock();
    if let Some(position) = entries.iter().position(|entry| entry.id == id) {
      entries.swap_remove(position);
    }
  }

  fn publish(&self, event: &SchedulerDiagnosticsEvent) -> PublishOutcome {
    let entries = self.entries.lock();
    if entries.is_empty() {
      return PublishOutcome { delivered: false, dropped: false };
    }
    let mut dropped = false;
    for entry in entries.iter() {
      if entry.buffer.push(event) {
        dropped = true;
      }
    }
    PublishOutcome { delivered: true, dropped }
  }
}

struct DiagnosticsSubscriber {
  id:     u64,
  buffer: ArcShared<DiagnosticsBuffer>,
}

struct DiagnosticsBuffer {
  queue:    NoStdMutex<VecDeque<SchedulerDiagnosticsEvent>>,
  capacity: usize,
}

impl DiagnosticsBuffer {
  fn new(capacity: usize) -> Self {
    Self { queue: NoStdMutex::new(VecDeque::new()), capacity }
  }

  fn push(&self, event: &SchedulerDiagnosticsEvent) -> bool {
    let mut guard = self.queue.lock();
    let mut dropped = false;
    if guard.len() >= self.capacity {
      guard.pop_front();
      dropped = true;
    }
    guard.push_back(event.clone());
    dropped
  }

  fn drain(&self) -> Vec<SchedulerDiagnosticsEvent> {
    let mut guard = self.queue.lock();
    guard.drain(..).collect()
  }
}

struct PublishOutcome {
  delivered: bool,
  dropped:   bool,
}

/// Event emitted through the diagnostics stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerDiagnosticsEvent {
  /// Job registration event with the scheduled deadline.
  Scheduled {
    /// Handle identifier.
    handle_id: u64,
    /// Deadline tick assigned during registration.
    deadline_tick: u64,
    /// Scheduling mode.
    mode:        SchedulerMode,
  },
  /// Job execution event including batch metadata.
  Fired {
    /// Handle identifier.
    handle_id: u64,
    /// Tick when the job fired.
    fired_tick: u64,
    /// Execution batch metadata.
    batch:      ExecutionBatch,
  },
  /// Job cancellation notification.
  Cancelled {
    /// Handle identifier.
    handle_id: u64,
    /// Tick when the cancellation was recorded.
    cancelled_tick: u64,
  },
  /// Warning emitted by the scheduler.
  Warning {
    /// Warning payload.
    warning: SchedulerWarning,
  },
}

/// Handle returned to diagnostics subscribers for draining events.
pub struct SchedulerDiagnosticsSubscription {
  id:       u64,
  registry: DiagnosticsRegistry,
  buffer:   ArcShared<DiagnosticsBuffer>,
  detached: bool,
}

impl SchedulerDiagnosticsSubscription {
  fn new(id: u64, registry: DiagnosticsRegistry, buffer: ArcShared<DiagnosticsBuffer>) -> Self {
    Self { id, registry, buffer, detached: false }
  }

  /// Drains and returns all pending diagnostics events.
  #[must_use]
  pub fn drain(&mut self) -> Vec<SchedulerDiagnosticsEvent> {
    self.buffer.drain()
  }
}

impl Drop for SchedulerDiagnosticsSubscription {
  fn drop(&mut self) {
    if !self.detached {
      self.registry.remove(self.id);
      self.detached = true;
    }
  }
}
