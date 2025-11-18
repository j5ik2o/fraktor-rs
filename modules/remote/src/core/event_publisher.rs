//! Publishes remoting lifecycle/backpressure events to the actor system.

#[cfg(test)]
mod tests;

use alloc::string::String;

use fraktor_actor_rs::core::{
  event_stream::{
    BackpressureSignal, CorrelationId, EventStreamEvent, RemotingBackpressureEvent, RemotingLifecycleEvent,
  },
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Helper that publishes remoting observability events.
pub struct EventPublisher<TB: RuntimeToolbox + 'static> {
  system: ActorSystemGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for EventPublisher<TB> {
  fn clone(&self) -> Self {
    Self { system: self.system.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> EventPublisher<TB> {
  /// Creates a new publisher bound to the provided actor system.
  #[must_use]
  pub fn new(system: ActorSystemGeneric<TB>) -> Self {
    Self { system }
  }

  /// Emits a `ListenStarted` lifecycle event.
  pub fn publish_listen_started(&self, authority: impl Into<String>, correlation_id: CorrelationId) {
    self.publish_lifecycle(RemotingLifecycleEvent::ListenStarted { authority: authority.into(), correlation_id });
  }

  /// Emits a `Connected` lifecycle event.
  pub fn publish_connected(
    &self,
    authority: impl Into<String>,
    remote_system: impl Into<String>,
    remote_uid: u64,
    correlation_id: CorrelationId,
  ) {
    self.publish_lifecycle(RemotingLifecycleEvent::Connected {
      authority: authority.into(),
      remote_system: remote_system.into(),
      remote_uid,
      correlation_id,
    });
  }

  /// Emits a `Quarantined` lifecycle event.
  pub fn publish_quarantined(
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

  /// Emits a `Gated` lifecycle event.
  pub fn publish_gated(&self, authority: impl Into<String>, correlation_id: CorrelationId) {
    self.publish_lifecycle(RemotingLifecycleEvent::Gated { authority: authority.into(), correlation_id });
  }

  /// Emits an arbitrary lifecycle event (e.g., Starting/Started/Shutdown).
  pub fn publish_lifecycle(&self, event: RemotingLifecycleEvent) {
    self.system.publish_event(&EventStreamEvent::RemotingLifecycle(event));
  }

  /// Emits a backpressure event for the provided authority.
  pub fn publish_backpressure(
    &self,
    authority: impl Into<String>,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
  ) {
    let event = RemotingBackpressureEvent::new(authority, signal, correlation_id);
    self.system.publish_event(&EventStreamEvent::RemotingBackpressure(event));
  }
}
