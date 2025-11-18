//! Publishes remoting lifecycle and backpressure events.

use alloc::string::{String, ToString};
use core::sync::atomic::{AtomicU64, Ordering};

use fraktor_actor_rs::core::event_stream::{
  BackpressureSignal, CorrelationId, EventStreamEvent, EventStreamGeneric, RemotingBackpressureEvent,
  RemotingLifecycleEvent,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::endpoint_manager::RemoteNodeId;

/// Emits remoting events through the actor system event stream.
pub(crate) struct EventPublisher<TB: RuntimeToolbox + 'static> {
  stream: ArcShared<EventStreamGeneric<TB>>,
  seed:   AtomicU64,
}

impl<TB: RuntimeToolbox + 'static> EventPublisher<TB> {
  /// Creates a new publisher bound to the provided stream.
  pub(crate) fn new(stream: ArcShared<EventStreamGeneric<TB>>) -> Self {
    Self { stream, seed: AtomicU64::new(1) }
  }

  /// Allocates a new correlation identifier.
  pub(crate) fn next_correlation_id(&self) -> CorrelationId {
    let raw = self.seed.fetch_add(1, Ordering::Relaxed);
    let hi = raw.rotate_left(13) ^ 0xA5A5_A5A5_A5A5_A5A5;
    let lo = raw as u32 ^ 0x5A5A_5A5A;
    CorrelationId::new(hi, lo)
  }

  /// Publishes a listen started event.
  pub(crate) fn lifecycle_listen_started(&self, authority: impl Into<String>, correlation_id: CorrelationId) {
    self.publish_lifecycle(RemotingLifecycleEvent::ListenStarted { authority: authority.into(), correlation_id });
  }

  /// Publishes a connected lifecycle event.
  pub(crate) fn lifecycle_connected(
    &self,
    authority: impl Into<String>,
    remote: &RemoteNodeId,
    correlation_id: CorrelationId,
  ) {
    self.publish_lifecycle(RemotingLifecycleEvent::Connected {
      authority: authority.into(),
      remote_system: remote.system().to_string(),
      remote_uid: remote.uid(),
      correlation_id,
    });
  }

  /// Publishes a quarantine lifecycle event.
  pub(crate) fn lifecycle_quarantined(
    &self,
    authority: impl Into<String>,
    reason: impl Into<String>,
    correlation_id: CorrelationId,
  ) {
    self.publish_lifecycle(RemotingLifecycleEvent::Quarantined {
      authority: authority.into(),
      reason: reason.into(),
      correlation_id,
    });
  }

  /// Publishes a gated lifecycle event.
  pub(crate) fn lifecycle_gated(&self, authority: impl Into<String>, correlation_id: CorrelationId) {
    self.publish_lifecycle(RemotingLifecycleEvent::Gated { authority: authority.into(), correlation_id });
  }

  /// Publishes a starting lifecycle event.
  pub(crate) fn lifecycle_starting(&self) {
    self.publish_lifecycle(RemotingLifecycleEvent::Starting);
  }

  /// Publishes a shutdown lifecycle event.
  pub(crate) fn lifecycle_shutdown(&self) {
    self.publish_lifecycle(RemotingLifecycleEvent::Shutdown);
  }

  /// Publishes an error lifecycle event.
  pub(crate) fn lifecycle_error(&self, message: impl Into<String>) {
    self.publish_lifecycle(RemotingLifecycleEvent::Error { message: message.into() });
  }

  /// Publishes a backpressure event for the authority.
  pub(crate) fn backpressure(
    &self,
    authority: impl Into<String>,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
  ) {
    let event = RemotingBackpressureEvent::new(authority, signal, correlation_id);
    self.stream.publish(&EventStreamEvent::RemotingBackpressure(event));
  }

  fn publish_lifecycle(&self, event: RemotingLifecycleEvent) {
    self.stream.publish(&EventStreamEvent::RemotingLifecycle(event));
  }
}

#[cfg(test)]
mod tests {
  use alloc::vec::Vec;

  use fraktor_actor_rs::core::event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, RemotingLifecycleEvent,
  };
  use fraktor_utils_rs::core::{
    runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
    sync::ArcShared,
  };

  use super::EventPublisher;

  struct RecordingSubscriber {
    events: ToolboxMutex<Vec<EventStreamEvent<NoStdToolbox>>, NoStdToolbox>,
  }

  impl RecordingSubscriber {
    fn new() -> ArcShared<Self> {
      ArcShared::new(Self {
        events: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      })
    }
  }

  impl EventStreamSubscriber for RecordingSubscriber {
    fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
      self.events.lock().push(event.clone());
    }
  }

  #[test]
  fn listen_started_event_contains_authority_and_correlation_id() {
    let stream = ArcShared::new(EventStreamGeneric::default());
    let subscriber = RecordingSubscriber::new();
    let subscriber_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
    let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber_ref);
    let publisher = EventPublisher::new(stream);
    let correlation = publisher.next_correlation_id();

    publisher.lifecycle_listen_started("127.0.0.1:2552", correlation);

    assert!(subscriber.events.lock().iter().any(|event| match event {
      | EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::ListenStarted { authority, correlation_id }) => {
        authority == "127.0.0.1:2552" && *correlation_id == correlation
      },
      | _ => false,
    }));
  }

  #[test]
  fn backpressure_event_propagates_signal_and_correlation_id() {
    let stream = ArcShared::new(EventStreamGeneric::default());
    let subscriber = RecordingSubscriber::new();
    let subscriber_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
    let _subscription = EventStreamGeneric::subscribe_arc(&stream, &subscriber_ref);
    let publisher = EventPublisher::new(stream);
    let correlation = publisher.next_correlation_id();

    publisher.backpressure("node-a", BackpressureSignal::Apply, correlation);

    assert!(subscriber.events.lock().iter().any(|event| match event {
      | EventStreamEvent::RemotingBackpressure(backpressure) => {
        backpressure.authority() == "node-a"
          && matches!(backpressure.signal(), BackpressureSignal::Apply)
          && backpressure.correlation_id() == correlation
      },
      | _ => false,
    }));
  }
}
