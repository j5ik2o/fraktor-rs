//! Event variants delivered through the event stream.

use crate::{DeadletterEntry, LifecycleEvent, LogEvent, MailboxMetricsEvent, RuntimeToolbox};

/// Event selected for publication on the event stream.
#[derive(Debug)]
pub enum EventStreamEvent<TB: RuntimeToolbox> {
  /// Actor lifecycle transition notification.
  Lifecycle(LifecycleEvent),
  /// Deadletter capture describing an undeliverable message.
  Deadletter(DeadletterEntry<TB>),
  /// Structured log event.
  Log(LogEvent),
  /// Mailbox metrics snapshot.
  Mailbox(MailboxMetricsEvent),
}

impl<TB: RuntimeToolbox> Clone for EventStreamEvent<TB> {
  fn clone(&self) -> Self {
    match self {
      | Self::Lifecycle(event) => Self::Lifecycle(event.clone()),
      | Self::Deadletter(entry) => Self::Deadletter(entry.clone()),
      | Self::Log(event) => Self::Log(event.clone()),
      | Self::Mailbox(event) => Self::Mailbox(event.clone()),
    }
  }
}
