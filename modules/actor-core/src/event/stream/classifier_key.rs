//! Event classifier keys used by subchannel subscriptions.

#[cfg(test)]
mod tests;

use crate::event::stream::EventStreamEvent;

/// Subscription classifier derived from the concrete [`EventStreamEvent`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassifierKey {
  /// Actor lifecycle transition notification.
  Lifecycle,
  /// Structured log event.
  Log,
  /// Deadletter capture describing an undeliverable message.
  DeadLetter,
  /// Extension-provided event namespaced by extension identifier.
  Extension,
  /// Mailbox metrics snapshot.
  Mailbox,
  /// Mailbox capacity pressure notification.
  MailboxPressure,
  /// Unhandled message notification from actor behaviors.
  UnhandledMessage,
  /// Message adapter failure notification.
  AdapterFailure,
  /// Serialization failure notification.
  Serialization,
  /// Remote authority state transition notification.
  RemoteAuthority,
  /// Backpressure notifications emitted by remoting transports.
  RemotingBackpressure,
  /// Remoting lifecycle change notification.
  RemotingLifecycle,
  /// Scheduler tick metrics snapshot.
  SchedulerTick,
  /// Tick driver activation snapshot.
  TickDriver,
  /// Wildcard classifier that subscribes to every event variant.
  All,
}

impl ClassifierKey {
  /// Returns the concrete classifier associated with an event variant.
  ///
  /// This method never returns [`Self::All`].
  #[must_use]
  pub const fn for_event(event: &EventStreamEvent) -> Self {
    match event {
      | EventStreamEvent::Lifecycle(_) => Self::Lifecycle,
      | EventStreamEvent::DeadLetter(_) => Self::DeadLetter,
      | EventStreamEvent::Log(_) => Self::Log,
      | EventStreamEvent::Mailbox(_) => Self::Mailbox,
      | EventStreamEvent::MailboxPressure(_) => Self::MailboxPressure,
      | EventStreamEvent::UnhandledMessage(_) => Self::UnhandledMessage,
      | EventStreamEvent::AdapterFailure(_) => Self::AdapterFailure,
      | EventStreamEvent::Serialization(_) => Self::Serialization,
      | EventStreamEvent::RemoteAuthority(_) => Self::RemoteAuthority,
      | EventStreamEvent::RemotingBackpressure(_) => Self::RemotingBackpressure,
      | EventStreamEvent::RemotingLifecycle(_) => Self::RemotingLifecycle,
      | EventStreamEvent::SchedulerTick(_) => Self::SchedulerTick,
      | EventStreamEvent::TickDriver(_) => Self::TickDriver,
      | EventStreamEvent::Extension { .. } => Self::Extension,
    }
  }
}
