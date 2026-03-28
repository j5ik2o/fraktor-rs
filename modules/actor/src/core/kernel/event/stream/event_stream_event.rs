//! Event variants delivered through the event stream.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::{
  adapter_failure_event::AdapterFailureEvent, remote_authority_event::RemoteAuthorityEvent,
  remoting_backpressure_event::RemotingBackpressureEvent, remoting_lifecycle_event::RemotingLifecycleEvent,
  tick_driver_snapshot::TickDriverSnapshot, typed_unhandled_message_event::TypedUnhandledMessageEvent,
};
use crate::core::kernel::{
  dead_letter::DeadLetterEntry,
  dispatch::{
    dispatcher::DispatcherDumpEvent,
    mailbox::metrics_event::{MailboxMetricsEvent, MailboxPressureEvent},
  },
  event::logging::LogEvent,
  lifecycle::LifecycleEvent,
  messaging::AnyMessage,
  scheduler::tick_driver::SchedulerTickMetrics,
  serialization::SerializationErrorEvent,
};

/// Event selected for publication on the event stream.
#[derive(Debug)]
pub enum EventStreamEvent {
  /// Actor lifecycle transition notification.
  Lifecycle(LifecycleEvent),
  /// Deadletter capture describing an undeliverable message.
  DeadLetter(DeadLetterEntry),
  /// Structured log event.
  Log(LogEvent),
  /// Mailbox metrics snapshot.
  Mailbox(MailboxMetricsEvent),
  /// Mailbox capacity pressure notification.
  MailboxPressure(MailboxPressureEvent),
  /// Dispatcher diagnostic snapshot.
  DispatcherDump(DispatcherDumpEvent),
  /// Unhandled message notification from typed behaviors.
  UnhandledMessage(TypedUnhandledMessageEvent),
  /// Message adapter failure notification.
  AdapterFailure(AdapterFailureEvent),
  /// Serialization failure notification.
  Serialization(SerializationErrorEvent),
  /// Remote authority state transition notification.
  RemoteAuthority(RemoteAuthorityEvent),
  /// Backpressure notifications emitted by remoting transports.
  RemotingBackpressure(RemotingBackpressureEvent),
  /// Remoting lifecycle change notification.
  RemotingLifecycle(RemotingLifecycleEvent),
  /// Scheduler tick metrics snapshot.
  SchedulerTick(SchedulerTickMetrics),
  /// Tick driver activation snapshot.
  TickDriver(TickDriverSnapshot),
  /// Extension-provided event namespaced by extension identifier.
  Extension {
    /// Extension identifier (e.g. "cluster").
    name:    String,
    /// Payload carried by the extension event.
    payload: AnyMessage,
  },
}

impl Clone for EventStreamEvent {
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
      | Self::RemotingBackpressure(event) => Self::RemotingBackpressure(event.clone()),
      | Self::RemotingLifecycle(event) => Self::RemotingLifecycle(event.clone()),
      | Self::SchedulerTick(event) => Self::SchedulerTick(event.clone()),
      | Self::TickDriver(event) => Self::TickDriver(event.clone()),
      | Self::Extension { name, payload } => Self::Extension { name: name.clone(), payload: payload.clone() },
    }
  }
}
