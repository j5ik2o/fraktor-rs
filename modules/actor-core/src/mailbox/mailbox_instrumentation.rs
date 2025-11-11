//! Mailbox metrics instrumentation and warning emission.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};

use fraktor_utils_core_rs::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::BackpressurePublisherGeneric;
use crate::{
  RuntimeToolbox,
  actor_prim::Pid,
  event_stream::EventStreamEvent,
  logging::LogLevel,
  mailbox::{MailboxMetricsEvent, MailboxPressureEvent},
  system::SystemStateGeneric,
};

const PRESSURE_THRESHOLD_PERCENT: usize = 75;

/// Provides mailbox metrics publication facilities.
#[derive(Clone)]
pub struct MailboxInstrumentationGeneric<TB: RuntimeToolbox + 'static> {
  system_state:   ArcShared<SystemStateGeneric<TB>>,
  capacity:       Option<usize>,
  throughput:     Option<usize>,
  warn_threshold: Option<usize>,
  pid:            Pid,
  backpressure:   Option<BackpressurePublisherGeneric<TB>>,
}

/// Type alias for the default mailbox instrumentation.
pub type MailboxInstrumentation = MailboxInstrumentationGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MailboxInstrumentationGeneric<TB> {
  /// Creates a new instrumentation helper.
  #[must_use]
  pub const fn new(
    system_state: ArcShared<SystemStateGeneric<TB>>,
    pid: Pid,
    capacity: Option<usize>,
    throughput: Option<usize>,
    warn_threshold: Option<usize>,
  ) -> Self {
    Self { system_state, capacity, throughput, warn_threshold, pid, backpressure: None }
  }

  /// Publishes a metrics snapshot.
  pub fn publish(&self, user_len: usize, system_len: usize) {
    let timestamp = self.system_state.monotonic_now();
    let event = MailboxMetricsEvent::new(self.pid, user_len, system_len, self.capacity, self.throughput, timestamp);
    self.system_state.publish_event(&EventStreamEvent::Mailbox(event));
    self.publish_pressure(user_len, timestamp);

    if let Some(threshold) = self.warn_threshold
      && user_len >= threshold
    {
      let message = format!("mailbox backlog reached {} (threshold: {})", user_len, threshold);
      self.system_state.emit_log(LogLevel::Warn, message, Some(self.pid));
    }
  }

  fn publish_pressure(&self, user_len: usize, timestamp: core::time::Duration) {
    let Some(capacity) = self.capacity else {
      return;
    };
    if capacity == 0 {
      return;
    }

    let utilization = ((user_len.saturating_mul(100)) / capacity).min(100) as u8;
    if utilization as usize >= PRESSURE_THRESHOLD_PERCENT {
      let event = MailboxPressureEvent::new(self.pid, user_len, capacity, utilization, timestamp, self.warn_threshold);
      self.system_state.publish_event(&EventStreamEvent::MailboxPressure(event.clone()));
      self.forward_backpressure(&event);
    }
  }

  /// Registers the dispatcher-facing publisher that consumes pressure events.
  pub fn attach_backpressure_publisher(&mut self, publisher: BackpressurePublisherGeneric<TB>) {
    self.backpressure = Some(publisher);
  }

  fn forward_backpressure(&self, event: &MailboxPressureEvent) {
    if let Some(publisher) = &self.backpressure {
      publisher.publish(event);
    }
  }

  /// Returns the associated system state handle.
  #[must_use]
  pub fn system_state(&self) -> ArcShared<SystemStateGeneric<TB>> {
    self.system_state.clone()
  }

  /// Emits a log event tagged with the owning actor pid.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>) {
    self.system_state.emit_log(level, message.into(), Some(self.pid));
  }

  /// Returns the pid associated with this mailbox.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }
}
