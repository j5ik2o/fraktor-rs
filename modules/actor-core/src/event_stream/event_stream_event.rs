//! Event variants delivered through the event stream.

#[cfg(test)]
mod tests;

use crate::{
  NoStdToolbox, RuntimeToolbox, dead_letter::DeadLetterEntryGeneric, lifecycle::LifecycleEvent, logging::LogEvent,
  mailbox::MailboxMetricsEvent,
};

/// Event selected for publication on the event stream.
#[derive(Debug)]
pub enum EventStreamEvent<TB: RuntimeToolbox = NoStdToolbox> {
  /// Actor lifecycle transition notification.
  Lifecycle(LifecycleEvent),
  /// Deadletter capture describing an undeliverable message.
  DeadLetter(DeadLetterEntryGeneric<TB>),
  /// Structured log event.
  Log(LogEvent),
  /// Mailbox metrics snapshot.
  Mailbox(MailboxMetricsEvent),
}

impl<TB: RuntimeToolbox> Clone for EventStreamEvent<TB> {
  fn clone(&self) -> Self {
    match self {
      | Self::Lifecycle(event) => Self::Lifecycle(event.clone()),
      | Self::DeadLetter(entry) => Self::DeadLetter(entry.clone()),
      | Self::Log(event) => Self::Log(event.clone()),
      | Self::Mailbox(event) => Self::Mailbox(event.clone()),
    }
  }
}
