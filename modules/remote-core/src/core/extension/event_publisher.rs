//! Thin helper that posts [`RemotingLifecycleEvent`]s into the actor-core
//! event stream.

use alloc::string::String;
use core::{
  any::Any,
  fmt::{Debug, Formatter, Result as FmtResult},
};

use fraktor_actor_core_rs::core::kernel::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, RemotingLifecycleEvent},
  system::ActorSystemWeak,
};

/// Publishes remoting lifecycle events through the actor-core event stream.
///
/// Holds an [`ActorSystemWeak`] directly per design Decision 14 — no extra
/// abstraction layer (such as a `LifecycleEventSink` trait) is introduced
/// because the crate already depends on `fraktor-actor-core-rs`.
#[derive(Clone)]
pub struct EventPublisher {
  system: ActorSystemWeak,
}

impl Debug for EventPublisher {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("EventPublisher").finish_non_exhaustive()
  }
}

impl EventPublisher {
  /// Creates a new publisher wrapping the given weak actor system.
  #[must_use]
  pub const fn new(system: ActorSystemWeak) -> Self {
    Self { system }
  }

  /// Publishes a remoting lifecycle event.
  ///
  /// If the underlying actor system has been dropped, this is a no-op.
  pub fn publish_lifecycle(&self, event: RemotingLifecycleEvent) {
    if let Some(system) = self.system.upgrade() {
      system.publish_event(&EventStreamEvent::RemotingLifecycle(event));
    }
  }

  /// Publishes an extension event.
  ///
  /// If the underlying actor system has been dropped, this is a no-op.
  pub fn publish_extension<T>(&self, name: impl Into<String>, payload: T)
  where
    T: Any + Send + Sync + 'static, {
    if let Some(system) = self.system.upgrade() {
      system.publish_event(&EventStreamEvent::Extension { name: name.into(), payload: AnyMessage::new(payload) });
    }
  }
}
