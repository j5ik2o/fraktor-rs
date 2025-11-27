//! Concrete implementation of [`RemotingControl`] backed by the actor system.
use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};
use core::sync::atomic::{AtomicU64, Ordering};

use fraktor_actor_rs::core::{
  actor_prim::actor_path::ActorPathParts,
  event_stream::{BackpressureSignal, CorrelationId, RemotingLifecycleEvent},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  EndpointWriterGeneric,
  endpoint_reader::EndpointReaderGeneric,
  event_publisher::EventPublisherGeneric,
  flight_recorder::{RemotingFlightRecorder, RemotingFlightRecorderSnapshot},
  quarantine_reason::QuarantineReason,
  remote_authority_snapshot::RemoteAuthoritySnapshot,
  remoting_backpressure_listener::RemotingBackpressureListener,
  remoting_control::RemotingControl,
  remoting_error::RemotingError,
  remoting_extension_config::RemotingExtensionConfig,
  transport::{RemoteTransport, RemoteTransportShared, TransportBackpressureHook, TransportBackpressureHookShared},
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
  pub(crate) fn new(system: ActorSystemGeneric<TB>, config: RemotingExtensionConfig) -> Self {
    let mut listeners: Vec<ArcShared<dyn RemotingBackpressureListener>> = Vec::new();
    for listener in config.backpressure_listeners() {
      listeners.push(listener.clone());
    }
    let publisher = EventPublisherGeneric::new(system.clone());
    let inner = RemotingControlInner {
      system,
      event_publisher: publisher,
      _canonical_host: config.canonical_host().to_string(),
      _canonical_port: config.canonical_port(),
      state: <TB::MutexFamily as SyncMutexFamily>::create(RemotingLifecycleState::new()),
      listeners: <TB::MutexFamily as SyncMutexFamily>::create(listeners),
      snapshots: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      recorder: RemotingFlightRecorder::new(config.flight_recorder_capacity()),
      correlation_seq: AtomicU64::new(1),
      writer: <TB::MutexFamily as SyncMutexFamily>::create(None),
      reader: <TB::MutexFamily as SyncMutexFamily>::create(None),
      transport_ref: <TB::MutexFamily as SyncMutexFamily>::create(None),
      #[cfg(feature = "tokio-transport")]
      endpoint_driver: <TB::MutexFamily as SyncMutexFamily>::create(None),
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
  pub(crate) fn notify_system_shutdown(&self) {
    if self.inner.state.lock().mark_shutdown() {
      self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Shutdown);
    }
  }

  fn ensure_can_run(&self) -> Result<(), RemotingError> {
    let guard = self.inner.state.lock();
    guard.ensure_running()
  }

  /// Registers the transport instance used by the runtime.
  #[allow(dead_code)]
  pub(crate) fn register_transport(&self, transport: Box<dyn RemoteTransport>) {
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
    writer: ArcShared<EndpointWriterGeneric<TB>>,
    reader: ArcShared<EndpointReaderGeneric<TB>>,
  ) {
    *self.inner.writer.lock() = Some(writer);
    *self.inner.reader.lock() = Some(reader);
    let _ = self.inner.try_bootstrap_runtime();
  }

  fn register_listener_dyn(&self, listener: ArcShared<dyn RemotingBackpressureListener>) {
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

  fn notify_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation: Option<CorrelationId>) {
    let listeners = {
      let guard = self.inner.listeners.lock();
      guard.clone()
    };
    let correlation_id = correlation.unwrap_or_else(|| self.inner.next_correlation_id());
    self.inner.event_publisher.publish_backpressure(authority.to_string(), signal, correlation_id);
    self.inner.record_backpressure(authority, signal, correlation_id);
    for listener in listeners {
      listener.on_signal(signal, authority, correlation_id);
    }
  }

  pub(crate) fn backpressure_hook(&self) -> TransportBackpressureHookShared {
    ArcShared::new(NoStdMutex::new(Box::new(ControlBackpressureHook { control: self.clone() })))
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
  fn start(&self) -> Result<(), RemotingError> {
    {
      let mut guard = self.inner.state.lock();
      guard.transition_to_start()?;
    }
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Starting);
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Started);
    self.inner.try_bootstrap_runtime()?;
    Ok(())
  }

  fn associate(&self, _address: &ActorPathParts) -> Result<(), RemotingError> {
    self.ensure_can_run()
  }

  fn quarantine(&self, _authority: &str, _reason: &QuarantineReason) -> Result<(), RemotingError> {
    self.ensure_can_run()
  }

  fn shutdown(&self) -> Result<(), RemotingError> {
    let mut guard = self.inner.state.lock();
    guard.transition_to_shutdown()?;
    drop(guard);
    self.inner.event_publisher.publish_lifecycle(RemotingLifecycleEvent::Shutdown);
    Ok(())
  }

  fn register_backpressure_listener<L>(&self, listener: L)
  where
    L: RemotingBackpressureListener, {
    let concrete: ArcShared<L> = ArcShared::new(listener);
    let dyn_listener: ArcShared<dyn RemotingBackpressureListener> = concrete;
    self.register_listener_dyn(dyn_listener);
  }

  fn connections_snapshot(&self) -> Vec<RemoteAuthoritySnapshot> {
    self.inner.snapshots.lock().clone()
  }
}

struct RemotingControlInner<TB>
where
  TB: RuntimeToolbox + 'static, {
  system:          ActorSystemGeneric<TB>,
  event_publisher: EventPublisherGeneric<TB>,
  _canonical_host: String,
  _canonical_port: Option<u16>,
  state:           ToolboxMutex<RemotingLifecycleState, TB>,
  listeners:       ToolboxMutex<Vec<ArcShared<dyn RemotingBackpressureListener>>, TB>,
  snapshots:       ToolboxMutex<Vec<RemoteAuthoritySnapshot>, TB>,
  recorder:        RemotingFlightRecorder,
  correlation_seq: AtomicU64,
  writer:          ToolboxMutex<Option<ArcShared<EndpointWriterGeneric<TB>>>, TB>,
  reader:          ToolboxMutex<Option<ArcShared<EndpointReaderGeneric<TB>>>, TB>,
  transport_ref:   ToolboxMutex<Option<RemoteTransportShared<TB>>, TB>,
  #[cfg(feature = "tokio-transport")]
  endpoint_driver: ToolboxMutex<Option<crate::std::runtime::endpoint_driver::EndpointDriverHandle>, TB>,
}

impl<TB> RemotingControlInner<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn next_correlation_id(&self) -> CorrelationId {
    let seq = self.correlation_seq.fetch_add(1, Ordering::Relaxed) as u128;
    CorrelationId::from_u128(seq)
  }

  fn record_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    let millis = self.system.state().monotonic_now().as_millis() as u64;
    self.recorder.record_backpressure(authority.to_string(), signal, correlation_id, millis);
  }

  fn try_bootstrap_runtime(&self) -> Result<(), RemotingError> {
    #[cfg(feature = "tokio-transport")]
    {
      if !self.state.lock().is_running() {
        return Ok(());
      }
      let mut driver_guard = self.endpoint_driver.lock();
      if driver_guard.is_some() {
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
      if self._canonical_host.is_empty() {
        return Err(RemotingError::TransportUnavailable("canonical host not configured".into()));
      }
      let port = self
        ._canonical_port
        .ok_or_else(|| RemotingError::TransportUnavailable("canonical port not configured".into()))?;
      let config = crate::std::runtime::endpoint_driver::EndpointDriverConfig {
        system: self.system.clone(),
        writer,
        reader,
        transport,
        event_publisher: self.event_publisher.clone(),
        canonical_host: self._canonical_host.clone(),
        canonical_port: port,
        system_name: self.system.state().system_name(),
      };
      let handle = crate::std::runtime::endpoint_driver::EndpointDriver::spawn(config)
        .map_err(|error| RemotingError::TransportUnavailable(format!("{error:?}")))?;
      *driver_guard = Some(handle);
    }
    Ok(())
  }
}

struct RemotingLifecycleState {
  phase: LifecyclePhase,
}

impl RemotingLifecycleState {
  const fn new() -> Self {
    Self { phase: LifecyclePhase::Idle }
  }

  fn is_running(&self) -> bool {
    matches!(self.phase, LifecyclePhase::Running)
  }

  fn ensure_running(&self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Running => Ok(()),
      | LifecyclePhase::Idle => Err(RemotingError::NotStarted),
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  fn transition_to_start(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Idle => {
        self.phase = LifecyclePhase::Running;
        Ok(())
      },
      | LifecyclePhase::Running => Err(RemotingError::AlreadyStarted),
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  fn transition_to_shutdown(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | LifecyclePhase::Idle | LifecyclePhase::Running => {
        self.phase = LifecyclePhase::Stopped;
        Ok(())
      },
      | LifecyclePhase::Stopped => Err(RemotingError::AlreadyShutdown),
    }
  }

  fn mark_shutdown(&mut self) -> bool {
    if matches!(self.phase, LifecyclePhase::Stopped) {
      false
    } else {
      self.phase = LifecyclePhase::Stopped;
      true
    }
  }
}

enum LifecyclePhase {
  Idle,
  Running,
  Stopped,
}

struct ControlBackpressureHook<TB>
where
  TB: RuntimeToolbox + 'static, {
  control: RemotingControlHandle<TB>,
}

impl<TB> TransportBackpressureHook for ControlBackpressureHook<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn on_backpressure(&mut self, signal: BackpressureSignal, authority: &str, correlation_id: CorrelationId) {
    self.control.notify_backpressure(authority, signal, Some(correlation_id));
  }
}
