//! Mailbox metrics instrumentation and warning emission.

#[cfg(test)]
mod tests;

use alloc::format;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
    RuntimeToolbox, actor_prim::Pid, event_stream::EventStreamEvent, logging::LogLevel, mailbox::MailboxMetricsEvent,
    system::SystemState,
};

/// Provides mailbox metrics publication facilities.
#[derive(Clone)]
pub struct MailboxInstrumentation<TB: RuntimeToolbox + 'static> {
  system_state:   ArcShared<SystemState<TB>>,
  capacity:       Option<usize>,
  throughput:     Option<usize>,
  warn_threshold: Option<usize>,
  pid:            Pid,
}

impl<TB: RuntimeToolbox + 'static> MailboxInstrumentation<TB> {
  /// Creates a new instrumentation helper.
  #[must_use]
  pub const fn new(
    system_state: ArcShared<SystemState<TB>>,
    pid: Pid,
    capacity: Option<usize>,
    throughput: Option<usize>,
    warn_threshold: Option<usize>,
  ) -> Self {
    Self { system_state, capacity, throughput, warn_threshold, pid }
  }

  /// Publishes a metrics snapshot.
  pub fn publish(&self, user_len: usize, system_len: usize) {
    let timestamp = self.system_state.monotonic_now();
    let event = MailboxMetricsEvent::new(self.pid, user_len, system_len, self.capacity, self.throughput, timestamp);
    self.system_state.publish_event(&EventStreamEvent::Mailbox(event));

    if let Some(threshold) = self.warn_threshold
      && user_len >= threshold
    {
      let message = format!("mailbox backlog reached {} (threshold: {})", user_len, threshold);
      self.system_state.emit_log(LogLevel::Warn, message, Some(self.pid));
    }
  }
}
