//! Unified event enumeration published via the event stream.

use crate::{
  deadletter_entry::DeadletterEntry, lifecycle_event::LifecycleEvent, log_event::LogEvent,
  mailbox_metrics_event::MailboxMetricsEvent,
};

/// Event variants emitted by the runtime.
#[derive(Clone, Debug)]
pub enum EventStreamEvent {
  /// Actor lifecycle transition.
  Lifecycle(LifecycleEvent),
  /// Deadletter notification.
  Deadletter(DeadletterEntry),
  /// Log event emitted by the runtime or application.
  Log(LogEvent),
  /// Mailbox metrics snapshot.
  Mailbox(MailboxMetricsEvent),
}
