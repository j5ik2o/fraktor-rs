//! Event variants delivered through the event stream.

#[cfg(test)]
mod tests;

use super::remote_authority_event::RemoteAuthorityEvent;
use crate::{
  NoStdToolbox, RuntimeToolbox,
  dead_letter::DeadLetterEntryGeneric,
  dispatcher::DispatcherDumpEvent,
  lifecycle::LifecycleEvent,
  logging::LogEvent,
  mailbox::{MailboxMetricsEvent, MailboxPressureEvent},
  serialization::SerializationErrorEvent,
  scheduler::SchedulerTickMetrics,
  typed::{UnhandledMessageEvent, message_adapter::AdapterFailureEvent},
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
  /// Mailbox capacity pressure notification.
  MailboxPressure(MailboxPressureEvent),
  /// Dispatcher diagnostic snapshot.
  DispatcherDump(DispatcherDumpEvent),
  /// Unhandled message notification from typed behaviors.
  UnhandledMessage(UnhandledMessageEvent),
  /// Message adapter failure notification.
  AdapterFailure(AdapterFailureEvent),
  /// Serialization failure notification.
  Serialization(SerializationErrorEvent),
  /// Remote authority state transition notification.
  RemoteAuthority(RemoteAuthorityEvent),
  /// Scheduler tick metrics snapshot.
  SchedulerTick(SchedulerTickMetrics),
}

impl<TB: RuntimeToolbox> Clone for EventStreamEvent<TB> {
  fn clone(&self) -> Self {
    match self {
      | Self::Lifecycle(event) => Self::Lifecycle(event.clone()),
      | Self::DeadLetter(entry) => Self::DeadLetter(entry.clone()),
      | Self::Log(event) => Self::Log(event.clone()),
      | Self::Mailbox(event) => Self::Mailbox(event.clone()),
      | Self::MailboxPressure(event) => Self::MailboxPressure(event.clone()),
      | Self::DispatcherDump(event) => Self::DispatcherDump(event.clone()),
      | Self::UnhandledMessage(event) => Self::UnhandledMessage(event.clone()),
      | Self::AdapterFailure(event) => Self::AdapterFailure(event.clone()),
      | Self::Serialization(event) => Self::Serialization(event.clone()),
      | Self::RemoteAuthority(event) => Self::RemoteAuthority(event.clone()),
      | Self::SchedulerTick(event) => Self::SchedulerTick(event.clone()),
    }
  }
}
