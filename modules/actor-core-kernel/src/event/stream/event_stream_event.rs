//! Event variants delivered through the event stream.

#[cfg(test)]
#[path = "event_stream_event_test.rs"]
mod tests;

use alloc::string::String;

use super::{
  adapter_failure_event::AdapterFailureEvent, address_terminated_event::AddressTerminatedEvent,
  remote_authority_event::RemoteAuthorityEvent, remoting_backpressure_event::RemotingBackpressureEvent,
  remoting_lifecycle_event::RemotingLifecycleEvent, tick_driver_snapshot::TickDriverSnapshot,
  unhandled_message_event::UnhandledMessageEvent,
};
use crate::{
  actor::{
    actor_ref::dead_letter::DeadLetterEntry, lifecycle::LifecycleEvent, messaging::AnyMessage,
    scheduler::tick_driver::SchedulerTickMetrics,
  },
  dispatch::mailbox::metrics_event::{MailboxMetricsEvent, MailboxPressureEvent},
  event::logging::LogEvent,
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
  /// Unhandled message notification from actor behaviors.
  UnhandledMessage(UnhandledMessageEvent),
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
  /// Remote node-level address termination notification.
  AddressTerminated(AddressTerminatedEvent),
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
      | Self::UnhandledMessage(event) => Self::UnhandledMessage(event.clone()),
      | Self::AdapterFailure(event) => Self::AdapterFailure(event.clone()),
      | Self::Serialization(event) => Self::Serialization(event.clone()),
      | Self::RemoteAuthority(event) => Self::RemoteAuthority(event.clone()),
      | Self::RemotingBackpressure(event) => Self::RemotingBackpressure(event.clone()),
      | Self::RemotingLifecycle(event) => Self::RemotingLifecycle(event.clone()),
      | Self::AddressTerminated(event) => Self::AddressTerminated(event.clone()),
      | Self::SchedulerTick(event) => Self::SchedulerTick(event.clone()),
      | Self::TickDriver(event) => Self::TickDriver(event.clone()),
      | Self::Extension { name, payload } => Self::Extension { name: name.clone(), payload: payload.clone() },
    }
  }
}
