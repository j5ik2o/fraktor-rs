//! Concrete implementation of [`RemotingControl`] backed by the actor system.
use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "tokio-transport")]
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::actor_path::ActorPathParts,
  event::stream::{BackpressureSignal, CorrelationId, RemotingLifecycleEvent},
  system::{ActorSystemGeneric, ActorSystemWeakGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{
  config::RemotingExtensionConfig, control::RemotingControl, control_backpressure_hook::ControlBackpressureHook,
  error::RemotingError, lifecycle_state::RemotingLifecycleState,
};
use crate::core::{
  actor_ref_provider::unregister_endpoint,
  backpressure::{RemotingBackpressureListener, RemotingBackpressureListenerShared},
  endpoint_association::QuarantineReason,
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::EndpointWriterSharedGeneric,
  event_publisher::EventPublisherGeneric,
  flight_recorder::{RemotingFlightRecorder, RemotingFlightRecorderSnapshot},
  remote_authority_snapshot::RemoteAuthoritySnapshot,
  transport::{RemoteTransport, RemoteTransportShared, TransportBackpressureHookShared},
};

/// Shared handle used by endpoints and providers to drive remoting.
pub struct RemotingControlHandle<TB>
where
  TB: RuntimeToolbox + 'static, {
  inner: ArcShared<RemotingControlInner<TB>>,
}

impl<TB> Clone for RemotingControlHandle<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB> RemotingControlHandle<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new handle bound to the provided actor system.
  ///
  /// The handle stores a weak reference to the actor system to avoid circular references.
  #[allow(dead_code)]
  pub(crate) fn new(system: ActorSystemGeneric<TB>, config: RemotingExtensionConfig) -> Self {
    let mut listeners: Vec<RemotingBackpressureListenerShared<TB>> = Vec::new();
    for listener in config.backpressure_listeners() {
      let boxed = listener.clone_box();
      listeners.push(ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(boxed)));
    }
    let system_weak = system.downgrade();
    let publisher = EventPublisherGeneric::new(system_weak.clone());
    let inner = RemotingControlInner {
      system: system_weak,
      event_publisher: publisher,
      canonical_host: config.canonical_host().to_string(),
      canonical_port: config.canonical_port(),
      #[cfg(feature = "tokio-transport")]
      handshake_timeout: config.handshake_timeout(),
      state: <TB::MutexFamily as SyncMutexFamily>::create(RemotingLifecycleState::new()),
      listeners: <TB::MutexFamily as SyncMutexFamily>::create(listeners),
      snapshots: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      recorder: RemotingFlightRecorder::new(config.flight_recorder_capacity()),
      correlation_seq: AtomicU64::new(1),
      writer: <TB::MutexFamily as SyncMutexFamily>::create(None),
      reader: <TB::MutexFamily as SyncMutexFamily>::create(None),
      transport_ref: <TB::MutexFamily as SyncMutexFamily>::create(None),
      #[cfg(feature = "tokio-transport")]
      endpoint_bridge: <TB::MutexFamily as SyncMutexFamily>::create(None),
    };
    Self { inner: ArcShared::new(inner) }
  }

  /// Returns `true` when the handle reports a running remoting subsystem.
  #[must_use]
  pub fn is_running(&self) -> bool {
    let guard = self.inner.state.lock();
    guard.is_running()
  }

  /// Internal helper invoked by the termination hook actor.
  ///
  /// This method also unregisters the loopback endpoint from the static registry
  /// to prevent memory leaks when the actor system is shut down.
  #[allow(dead_code)]
  pub(crate) fn notify_system_shutdown(&self) {
    if self.inner.state.lock().mark_shutdown() {
      // Unregister loopback endpoint to clean up static registry
      let authority = self.inner.format_authority();
      unregister_endpoint(&authority);
      self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Shutdown);
    }
  }

  fn ensure_can_run(&self) -> Result<(), RemotingError> {
    let guard = self.inner.state.lock();
    guard.ensure_running()
  }

  /// Registers the transport instance used by the runtime.
  #[allow(dead_code)]
  pub(crate) fn register_transport(&self, transport: Box<dyn RemoteTransport<TB>>) {
    let shared: RemoteTransportShared<TB> = RemoteTransportShared::new(transport);
    self.register_remote_transport_shared(shared);
  }

  /// Registers a pre-wrapped shared transport instance.
  pub(crate) fn register_remote_transport_shared(&self, transport: RemoteTransportShared<TB>) {
    *self.inner.transport_ref.lock() = Some(transport);
    let _ = self.inner.try_bootstrap_runtime();
  }

  /// Registers endpoint IO components required for transport bridging.
  pub(crate) fn register_endpoint_io(
    &self,
    writer: EndpointWriterSharedGeneric<TB>,
    reader: ArcShared<EndpointReaderGeneric<TB>>,
  ) {
    *self.inner.writer.lock() = Some(writer);
    *self.inner.reader.lock() = Some(reader);
    let _ = self.inner.try_bootstrap_runtime();
  }

  fn register_listener_dyn(&self, listener: RemotingBackpressureListenerShared<TB>) {
    let mut guard = self.inner.listeners.lock();
    guard.push(listener);
  }

  pub(crate) fn record_authority_snapshot(&self, snapshot: RemoteAuthoritySnapshot) {
    let mut guard = self.inner.snapshots.lock();
    if let Some(existing) = guard.iter_mut().find(|entry| entry.authority() == snapshot.authority()) {
      *existing = snapshot;
    } else {
      guard.push(snapshot);
    }
  }

  pub(super) fn notify_backpressure(
    &self,
    authority: &str,
    signal: BackpressureSignal,
    correlation: Option<CorrelationId>,
  ) {
    let listeners = {
      let guard = self.inner.listeners.lock();
      guard.clone()
    };
    let correlation_id = correlation.unwrap_or_else(|| self.inner.next_correlation_id());
    self.inner.event_publisher.publish_backpressure(authority.to_string(), signal, correlation_id);
    self.inner.record_backpressure(authority, signal, correlation_id);
    for listener in listeners {
      let mut guard = listener.lock();
      guard.on_signal(signal, authority, correlation_id);
    }
  }

  #[allow(dead_code)]
  pub(crate) fn backpressure_hook(&self) -> TransportBackpressureHookShared {
    TransportBackpressureHookShared::new(Box::new(ControlBackpressureHook { control: self.clone() }))
  }

  /// Emits a synthetic backpressure signal for diagnostics.
  pub fn emit_backpressure_signal(&self, authority: &str, signal: BackpressureSignal) {
    self.notify_backpressure(authority, signal, None);
  }

  /// Returns the most recent flight recorder snapshot.
  #[must_use]
  pub fn flight_recorder_snapshot(&self) -> RemotingFlightRecorderSnapshot {
    self.inner.recorder.snapshot()
  }
}

impl<TB> RemotingControl<TB> for RemotingControlHandle<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn start(&mut self) -> Result<(), RemotingError> {
    {
      let mut guard = self.inner.state.lock();
      guard.transition_to_start()?;
    }
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Starting);
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Started);
    self.inner.try_bootstrap_runtime()?;
    Ok(())
  }

  fn associate(&mut self, _address: &ActorPathParts) -> Result<(), RemotingError> {
    self.ensure_can_run()
  }

  fn quarantine(&mut self, _authority: &str, _reason: &QuarantineReason) -> Result<(), RemotingError> {
    self.ensure_can_run()
  }

  fn shutdown(&mut self) -> Result<(), RemotingError> {
    let mut guard = self.inner.state.lock();
    guard.transition_to_shutdown()?;
    drop(guard);
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Shutdown);
    Ok(())
  }

  fn register_backpressure_listener<L>(&mut self, listener: L)
  where
    L: RemotingBackpressureListener, {
    let dyn_listener: RemotingBackpressureListenerShared<TB> =
      ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(Box::new(listener)));
    self.register_listener_dyn(dyn_listener);
  }

  fn connections_snapshot(&self) -> Vec<RemoteAuthoritySnapshot> {
    self.inner.snapshots.lock().clone()
  }
}

struct RemotingControlInner<TB>
where
  TB: RuntimeToolbox + 'static, {
  system:            ActorSystemWeakGeneric<TB>,
  event_publisher:   EventPublisherGeneric<TB>,
  canonical_host:    String,
  canonical_port:    Option<u16>,
  #[cfg(feature = "tokio-transport")]
  handshake_timeout: Duration,
  state:             ToolboxMutex<RemotingLifecycleState, TB>,
  listeners:         ToolboxMutex<Vec<RemotingBackpressureListenerShared<TB>>, TB>,
  snapshots:         ToolboxMutex<Vec<RemoteAuthoritySnapshot>, TB>,
  recorder:          RemotingFlightRecorder,
  correlation_seq:   AtomicU64,
  writer:            ToolboxMutex<Option<EndpointWriterSharedGeneric<TB>>, TB>,
  reader:            ToolboxMutex<Option<ArcShared<EndpointReaderGeneric<TB>>>, TB>,
  transport_ref:     ToolboxMutex<Option<RemoteTransportShared<TB>>, TB>,
  #[cfg(feature = "tokio-transport")]
  endpoint_bridge:   ToolboxMutex<Option<crate::std::endpoint_transport_bridge::EndpointTransportBridgeHandle>, TB>,
}

impl<TB> RemotingControlInner<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Formats the canonical authority string from host and port.
  fn format_authority(&self) -> String {
    match self.canonical_port {
      | Some(port) => format!("{}:{}", self.canonical_host, port),
      | None => self.canonical_host.clone(),
    }
  }

  fn next_correlation_id(&self) -> CorrelationId {
    let seq = self.correlation_seq.fetch_add(1, Ordering::Relaxed) as u128;
    CorrelationId::from_u128(seq)
  }

  fn record_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    let millis = self.system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0);
    self.recorder.record_backpressure(authority.to_string(), signal, correlation_id, millis);
  }

  fn try_bootstrap_runtime(&self) -> Result<(), RemotingError> {
    #[cfg(feature = "tokio-transport")]
    {
      if !self.state.lock().is_running() {
        return Ok(());
      }
      let mut bridge_guard = self.endpoint_bridge.lock();
      if bridge_guard.is_some() {
        return Ok(());
      }
      let Some(transport) = self.transport_ref.lock().clone() else {
        return Ok(());
      };
      let Some(writer) = self.writer.lock().clone() else {
        return Ok(());
      };
      let Some(reader) = self.reader.lock().clone() else {
        return Ok(());
      };
      let Some(system) = self.system.upgrade() else {
        return Err(RemotingError::TransportUnavailable("actor system has been dropped".into()));
      };
      if self.canonical_host.is_empty() {
        return Err(RemotingError::TransportUnavailable("canonical host not configured".into()));
      }
      let port = self
        .canonical_port
        .ok_or_else(|| RemotingError::TransportUnavailable("canonical port not configured".into()))?;
      let system_name = system.state().system_name();
      let config = crate::std::endpoint_transport_bridge::EndpointTransportBridgeConfig {
        system: system.downgrade(),
        writer,
        reader,
        transport,
        event_publisher: self.event_publisher.clone(),
        canonical_host: self.canonical_host.clone(),
        canonical_port: port,
        system_name,
        #[cfg(feature = "tokio-transport")]
        handshake_timeout: self.handshake_timeout,
      };
      let handle = crate::std::endpoint_transport_bridge::EndpointTransportBridge::spawn(config)
        .map_err(|error| RemotingError::TransportUnavailable(format!("{error:?}")))?;
      *bridge_guard = Some(handle);
    }
    Ok(())
  }
}
