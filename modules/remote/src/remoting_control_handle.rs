//! Concrete [`RemotingControl`] handle shared with runtime components.

use alloc::{format, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_actor_rs::core::{
  actor_prim::{actor_path::ActorPathParts, actor_ref::ActorRefGeneric},
  event_stream::{
    BackpressureSignal, EventStreamEvent, EventStreamGeneric, RemotingBackpressureEvent, RemotingLifecycleEvent,
  },
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::{
  RemoteTransport, RemotingBackpressureListener, RemotingConnectionSnapshot, RemotingControl, RemotingError,
  RemotingExtensionConfig, TransportFactory,
};

struct RemotingControlShared<TB: RuntimeToolbox + 'static> {
  _system:      ActorSystemGeneric<TB>,
  event_stream: ArcShared<EventStreamGeneric<TB>>,
  supervisor:   ToolboxMutex<Option<ActorRefGeneric<TB>>, TB>,
  listeners:    ToolboxMutex<Vec<ArcShared<dyn RemotingBackpressureListener>>, TB>,
  transport:    ToolboxMutex<Option<ArcShared<dyn RemoteTransport<TB>>>, TB>,
  config:       RemotingExtensionConfig,
  started:      AtomicBool,
  shutdown:     AtomicBool,
}

/// Shared handle implementing the [`RemotingControl`] interface.
pub struct RemotingControlHandle<TB: RuntimeToolbox + 'static> {
  shared: ArcShared<RemotingControlShared<TB>>,
}

impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  /// Creates a new handle bound to the provided actor system.
  #[must_use]
  pub(crate) fn new(system: &ActorSystemGeneric<TB>, config: RemotingExtensionConfig) -> Self {
    let shared = RemotingControlShared {
      _system: system.clone(),
      event_stream: system.event_stream(),
      supervisor: <TB::MutexFamily as SyncMutexFamily>::create(None),
      listeners: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      transport: <TB::MutexFamily as SyncMutexFamily>::create(None),
      config,
      started: AtomicBool::new(false),
      shutdown: AtomicBool::new(false),
    };
    Self { shared: ArcShared::new(shared) }
  }

  /// Assigns the endpoint supervisor actor reference.
  pub(crate) fn set_supervisor(&self, supervisor: ActorRefGeneric<TB>) {
    *self.shared.supervisor.lock() = Some(supervisor);
  }

  /// Exposes the supervisor actor reference for testing.
  #[cfg(test)]
  pub(crate) fn supervisor_ref(&self) -> Option<ActorRefGeneric<TB>> {
    self.shared.supervisor.lock().clone()
  }

  pub(crate) fn publish_shutdown(&self) {
    if !self.shared.shutdown.swap(true, Ordering::SeqCst) {
      self.publish_event(RemotingLifecycleEvent::Shutdown);
    }
  }

  fn publish_event(&self, event: RemotingLifecycleEvent) {
    let payload = EventStreamEvent::RemotingLifecycle(event);
    self.shared.event_stream.publish(&payload);
  }

  fn publish_backpressure(&self, event: RemotingBackpressureEvent) {
    self.shared.event_stream.publish(&EventStreamEvent::RemotingBackpressure(event));
  }

  fn notify_backpressure_internal(&self, signal: BackpressureSignal, authority: &str) {
    self.publish_backpressure(RemotingBackpressureEvent::new(authority, signal));
    let listeners = self.shared.listeners.lock().clone();
    for listener in listeners.iter() {
      listener.on_signal(signal, authority);
    }
  }

  #[cfg(test)]
  pub(crate) fn test_notify_backpressure(&self, signal: BackpressureSignal, authority: &str) {
    self.notify_backpressure_internal(signal, authority);
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for RemotingControlHandle<TB> {
  fn clone(&self) -> Self {
    Self { shared: self.shared.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> RemotingControl<TB> for RemotingControlHandle<TB> {
  fn start(&self) -> Result<(), RemotingError> {
    if self.shared.shutdown.load(Ordering::SeqCst) {
      return Err(RemotingError::SystemUnavailable);
    }
    if self.shared.started.swap(true, Ordering::SeqCst) {
      return Err(RemotingError::AlreadyStarted);
    }
    match TransportFactory::create::<TB>(&self.shared.config) {
      | Ok(transport) => {
        *self.shared.transport.lock() = Some(transport);
        self.publish_event(RemotingLifecycleEvent::Starting);
        Ok(())
      },
      | Err(error) => {
        self.shared.started.store(false, Ordering::SeqCst);
        self.publish_event(RemotingLifecycleEvent::Error { message: format!("{error}") });
        Err(error)
      },
    }
  }

  fn associate(&self, _address: &ActorPathParts) -> Result<(), RemotingError> {
    Err(RemotingError::Unsupported("associate"))
  }

  fn quarantine(&self, _authority: &str, _reason: &str) -> Result<(), RemotingError> {
    Err(RemotingError::Unsupported("quarantine"))
  }

  fn shutdown(&self) -> Result<(), RemotingError> {
    if !self.shared.started.load(Ordering::SeqCst) {
      return Err(RemotingError::NotStarted);
    }
    if self.shared.shutdown.swap(true, Ordering::SeqCst) {
      return Ok(());
    }
    self.publish_event(RemotingLifecycleEvent::Shutdown);
    Ok(())
  }

  fn register_backpressure_listener(&self, listener: ArcShared<dyn RemotingBackpressureListener>) {
    self.shared.listeners.lock().push(listener);
  }

  fn connections_snapshot(&self) -> Vec<RemotingConnectionSnapshot> {
    Vec::new()
  }
}

impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  #[allow(dead_code)]
  pub(crate) fn notify_backpressure(&self, signal: BackpressureSignal, authority: &str) {
    self.notify_backpressure_internal(signal, authority);
  }
}
