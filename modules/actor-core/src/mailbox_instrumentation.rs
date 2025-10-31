//! Instrumentation hook used by the mailbox to emit metrics.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  event_stream::EventStream, event_stream_event::EventStreamEvent, log_event::LogEvent, log_level::LogLevel,
  mailbox_metrics_event::MailboxMetricsEvent, pid::Pid, system_state::ActorSystemState,
};

/// Provides mailbox metrics publication facilities.
#[derive(Clone)]
pub struct MailboxInstrumentation {
  event_stream:   ArcShared<EventStream>,
  system_state:   ArcShared<ActorSystemState>,
  pid:            Pid,
  capacity:       Option<usize>,
  throughput:     Option<usize>,
  warn_threshold: Option<usize>,
}

impl MailboxInstrumentation {
  /// Creates a new instrumentation helper.
  #[must_use]
  pub const fn new(
    event_stream: ArcShared<EventStream>,
    system_state: ArcShared<ActorSystemState>,
    pid: Pid,
    capacity: Option<usize>,
    throughput: Option<usize>,
    warn_threshold: Option<usize>,
  ) -> Self {
    Self { event_stream, system_state, pid, capacity, throughput, warn_threshold }
  }

  /// Publishes a metrics snapshot.
  pub fn publish(&self, user_len: usize, system_len: usize) {
    let timestamp = self.system_state.monotonic_now();
    let event = MailboxMetricsEvent::new(self.pid, user_len, system_len, self.capacity, self.throughput, timestamp);
    self.event_stream.publish(EventStreamEvent::Mailbox(event));
    if let Some(threshold) = self.warn_threshold
      && user_len >= threshold
    {
      let message = alloc::format!("mailbox backlog reached {} (threshold: {})", user_len, threshold);
      let log = LogEvent::new(LogLevel::Warn, message, timestamp, Some(self.pid));
      self.event_stream.publish(EventStreamEvent::Log(log));
    }
  }
}
