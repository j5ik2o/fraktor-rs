//! Mailbox metrics instrumentation and warning emission.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};

use super::BackpressurePublisher;
use crate::core::kernel::{
  actor::Pid,
  dispatch::mailbox::metrics_event::{MailboxMetricsEvent, MailboxPressureEvent},
  event::{logging::LogLevel, stream::EventStreamEvent},
  system::state::{SystemStateShared, SystemStateWeak},
};

const PRESSURE_THRESHOLD_PERCENT: usize = 75;

/// Provides mailbox metrics publication facilities.
#[derive(Clone)]
pub struct MailboxInstrumentation {
  system_state:   SystemStateWeak,
  capacity:       Option<usize>,
  throughput:     Option<usize>,
  warn_threshold: Option<usize>,
  pid:            Pid,
  backpressure:   Option<BackpressurePublisher>,
}

impl MailboxInstrumentation {
  /// Creates a new instrumentation helper.
  #[must_use]
  #[allow(clippy::needless_pass_by_value)]
  pub fn new(
    system_state: SystemStateShared,
    pid: Pid,
    capacity: Option<usize>,
    throughput: Option<usize>,
    warn_threshold: Option<usize>,
  ) -> Self {
    Self { system_state: system_state.downgrade(), capacity, throughput, warn_threshold, pid, backpressure: None }
  }

  /// Upgrades the weak system state reference to a strong reference.
  fn get_system_state(&self) -> Option<SystemStateShared> {
    self.system_state.upgrade()
  }

  /// Publishes a metrics snapshot.
  pub fn publish(&self, user_len: usize, system_len: usize) {
    let Some(system_state) = self.get_system_state() else {
      return;
    };
    let timestamp = system_state.monotonic_now();
    let event = MailboxMetricsEvent::new(self.pid, user_len, system_len, self.capacity, self.throughput, timestamp);
    system_state.publish_event(&EventStreamEvent::Mailbox(event));
    self.publish_pressure(&system_state, user_len, timestamp);

    if let Some(threshold) = self.warn_threshold
      && user_len >= threshold
    {
      let message = format!("mailbox backlog reached {} (threshold: {})", user_len, threshold);
      system_state.emit_log(LogLevel::Warn, message, Some(self.pid));
    }
  }

  fn publish_pressure(&self, system_state: &SystemStateShared, user_len: usize, timestamp: core::time::Duration) {
    let Some(capacity) = self.capacity else {
      return;
    };
    if capacity == 0 {
      return;
    }

    let utilization = ((user_len.saturating_mul(100)) / capacity).min(100) as u8;
    if utilization as usize >= PRESSURE_THRESHOLD_PERCENT {
      let event = MailboxPressureEvent::new(self.pid, user_len, capacity, utilization, timestamp, self.warn_threshold);
      system_state.publish_event(&EventStreamEvent::MailboxPressure(event.clone()));
      self.forward_backpressure(&event);
    }
  }

  /// Registers the dispatcher-facing publisher that consumes pressure events.
  pub fn attach_backpressure_publisher(&mut self, publisher: BackpressurePublisher) {
    self.backpressure = Some(publisher);
  }

  fn forward_backpressure(&self, event: &MailboxPressureEvent) {
    if let Some(publisher) = &self.backpressure {
      publisher.publish(event);
    }
  }

  /// Returns the associated system state handle.
  #[must_use]
  pub fn system_state(&self) -> Option<SystemStateShared> {
    self.get_system_state()
  }

  /// Emits a log event tagged with the owning actor pid.
  pub fn emit_log(&self, level: LogLevel, message: impl Into<String>) {
    if let Some(system_state) = self.get_system_state() {
      system_state.emit_log(level, message.into(), Some(self.pid));
    }
  }

  /// Returns the pid associated with this mailbox.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }
}
