//! Concrete implementation of [`RemotingControl`] backed by the actor system.
#[cfg(test)]
mod tests;

#[cfg(feature = "tokio-transport")]
use alloc::collections::BTreeMap;
#[cfg(feature = "tokio-transport")]
use alloc::sync::Arc;
use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "tokio-transport")]
use core::time::Duration;

#[cfg(feature = "tokio-transport")]
use fraktor_actor_rs::core::messaging::AnyMessageGeneric;
use fraktor_actor_rs::core::{
  actor::{actor_path::ActorPathParts, actor_ref::ActorRefGeneric},
  event::stream::{BackpressureSignal, CorrelationId, RemotingLifecycleEvent},
  system::{ActorSystemGeneric, ActorSystemWeakGeneric},
};
#[cfg(any(feature = "tokio-transport", test, feature = "test-support"))]
use fraktor_utils_rs::core::sync::SharedAccess;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{
  config::RemotingExtensionConfig, control::RemotingControl, control_backpressure_hook::ControlBackpressureHook,
  error::RemotingError, lifecycle_state::RemotingLifecycleState,
};
#[cfg(feature = "tokio-transport")]
use crate::core::RemoteInstrument;
#[cfg(feature = "tokio-transport")]
use crate::core::transport::{TransportChannel, TransportEndpoint};
#[cfg(feature = "tokio-transport")]
use crate::core::watcher::{Heartbeat, RemoteWatcherCommand};
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
      #[cfg(feature = "tokio-transport")]
      shutdown_flush_timeout: config.shutdown_flush_timeout(),
      state: <TB::MutexFamily as SyncMutexFamily>::create(RemotingLifecycleState::new()),
      listeners: <TB::MutexFamily as SyncMutexFamily>::create(listeners),
      snapshots: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      recorder: RemotingFlightRecorder::new(config.flight_recorder_capacity()),
      correlation_seq: AtomicU64::new(1),
      writer: <TB::MutexFamily as SyncMutexFamily>::create(None),
      reader: <TB::MutexFamily as SyncMutexFamily>::create(None),
      watcher_daemon: <TB::MutexFamily as SyncMutexFamily>::create(None),
      transport_ref: <TB::MutexFamily as SyncMutexFamily>::create(None),
      #[cfg(feature = "tokio-transport")]
      remote_instruments: config.remote_instruments().to_vec(),
      #[cfg(feature = "tokio-transport")]
      endpoint_bridge: <TB::MutexFamily as SyncMutexFamily>::create(None),
      #[cfg(feature = "tokio-transport")]
      heartbeat_channels: <TB::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()),
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
    let _ = self.inner.try_bootstrap_runtime(self.clone());
  }

  /// Registers endpoint IO components required for transport bridging.
  pub(crate) fn register_endpoint_io(
    &self,
    writer: EndpointWriterSharedGeneric<TB>,
    reader: ArcShared<EndpointReaderGeneric<TB>>,
  ) {
    *self.inner.writer.lock() = Some(writer);
    *self.inner.reader.lock() = Some(reader);
    let _ = self.inner.try_bootstrap_runtime(self.clone());
  }

  /// Registers the remote watcher daemon actor.
  pub(crate) fn register_remote_watcher_daemon(&self, daemon: ActorRefGeneric<TB>) {
    *self.inner.watcher_daemon.lock() = Some(daemon);
  }

  /// Dispatches a command to the registered remote watcher daemon.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when the watcher daemon is not yet
  /// registered or when the tell operation fails.
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn dispatch_remote_watcher_command(&self, command: RemoteWatcherCommand) -> Result<(), RemotingError> {
    let daemon = self.inner.watcher_daemon.lock().clone();
    match daemon {
      | Some(daemon) => daemon
        .tell(AnyMessageGeneric::new(command))
        .map(|_| ())
        .map_err(|error| RemotingError::TransportUnavailable(format!("{error:?}"))),
      | None => Err(RemotingError::TransportUnavailable("watcher daemon not registered; command dropped".into())),
    }
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

  /// Binds a transport listener for tests using the currently registered transport.
  #[cfg(any(test, feature = "test-support"))]
  pub fn bind_transport_listener_for_test(
    &self,
    bind: &crate::core::transport::TransportBind,
  ) -> Result<(), RemotingError> {
    let transport = self
      .inner
      .transport_ref
      .lock()
      .clone()
      .ok_or_else(|| RemotingError::TransportUnavailable("remote transport not registered".to_string()))?;
    transport.with_write(|t| t.spawn_listener(bind)).map(|_| ()).map_err(RemotingError::from)
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
    self.inner.try_bootstrap_runtime(self.clone())?;
    Ok(())
  }

  fn associate(&mut self, address: &ActorPathParts) -> Result<(), RemotingError> {
    self.ensure_can_run()?;
    #[cfg(feature = "tokio-transport")]
    if let Some(authority) = address.authority_endpoint() {
      self.inner.send_heartbeat_probe(&authority)?;
    }
    #[cfg(not(feature = "tokio-transport"))]
    {
      let _ = address;
    }
    Ok(())
  }

  fn quarantine(&mut self, authority: &str, _reason: &QuarantineReason) -> Result<(), RemotingError> {
    self.ensure_can_run()?;
    #[cfg(feature = "tokio-transport")]
    {
      self.inner.heartbeat_channels.lock().remove(authority);
    }
    #[cfg(not(feature = "tokio-transport"))]
    {
      let _ = authority;
    }
    Ok(())
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
  system:                 ActorSystemWeakGeneric<TB>,
  event_publisher:        EventPublisherGeneric<TB>,
  canonical_host:         String,
  canonical_port:         Option<u16>,
  #[cfg(feature = "tokio-transport")]
  handshake_timeout:      Duration,
  #[cfg(feature = "tokio-transport")]
  shutdown_flush_timeout: Duration,
  state:                  ToolboxMutex<RemotingLifecycleState, TB>,
  listeners:              ToolboxMutex<Vec<RemotingBackpressureListenerShared<TB>>, TB>,
  snapshots:              ToolboxMutex<Vec<RemoteAuthoritySnapshot>, TB>,
  recorder:               RemotingFlightRecorder,
  correlation_seq:        AtomicU64,
  writer:                 ToolboxMutex<Option<EndpointWriterSharedGeneric<TB>>, TB>,
  reader:                 ToolboxMutex<Option<ArcShared<EndpointReaderGeneric<TB>>>, TB>,
  watcher_daemon:         ToolboxMutex<Option<ActorRefGeneric<TB>>, TB>,
  transport_ref:          ToolboxMutex<Option<RemoteTransportShared<TB>>, TB>,
  #[cfg(feature = "tokio-transport")]
  remote_instruments:     Vec<Arc<dyn RemoteInstrument>>,
  #[cfg(feature = "tokio-transport")]
  endpoint_bridge: ToolboxMutex<Option<crate::std::endpoint_transport_bridge::EndpointTransportBridgeHandle>, TB>,
  #[cfg(feature = "tokio-transport")]
  heartbeat_channels:     ToolboxMutex<BTreeMap<String, TransportChannel>, TB>,
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

  #[cfg(feature = "tokio-transport")]
  fn send_heartbeat_probe(&self, authority: &str) -> Result<(), RemotingError> {
    let transport = self
      .transport_ref
      .lock()
      .clone()
      .ok_or_else(|| RemotingError::TransportUnavailable("remote transport not registered".to_string()))?;
    // open_channel をロック内で実行しレース条件によるリソースリークを防ぐ
    let channel = {
      let mut channels = self.heartbeat_channels.lock();
      match channels.get(authority).copied() {
        | Some(ch) => ch,
        | None => {
          let endpoint = TransportEndpoint::new(authority.to_string());
          let opened = transport.with_write(|t| t.open_channel(&endpoint))?;
          channels.insert(authority.to_string(), opened);
          opened
        },
      }
    };
    let payload = Heartbeat::new(authority).encode_frame();
    // 送信失敗時はキャッシュを無効化して壊れたチャネルの再利用を防ぐ
    if let Err(error) = transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil())) {
      self.heartbeat_channels.lock().remove(authority);
      return Err(error.into());
    }
    Ok(())
  }

  fn record_backpressure(&self, authority: &str, signal: BackpressureSignal, correlation_id: CorrelationId) {
    let millis = self.system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0);
    self.recorder.record_backpressure(authority.to_string(), signal, correlation_id, millis);
  }

  fn try_bootstrap_runtime(&self, control: RemotingControlHandle<TB>) -> Result<(), RemotingError> {
    #[cfg(feature = "tokio-transport")]
    {
      if !self.state.lock().is_running() {
        return Ok(());
      }
      // transport_ref, writer, reader を先にclone取得してからendpoint_bridgeをロックする。
      // register_endpoint_io()がwriter/readerロック後にこの関数を呼ぶため、
      // 逆順でendpoint_bridgeを先にロックするとABBA型デッドロックになる
      let Some(transport) = self.transport_ref.lock().clone() else {
        return Ok(());
      };
      let Some(writer) = self.writer.lock().clone() else {
        return Ok(());
      };
      let Some(reader) = self.reader.lock().clone() else {
        return Ok(());
      };
      let mut bridge_guard = self.endpoint_bridge.lock();
      if bridge_guard.is_some() {
        return Ok(());
      }
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
        control,
        writer,
        reader,
        transport,
        event_publisher: self.event_publisher.clone(),
        canonical_host: self.canonical_host.clone(),
        canonical_port: port,
        system_name,
        remote_instruments: self.remote_instruments.clone(),
        #[cfg(feature = "tokio-transport")]
        handshake_timeout: self.handshake_timeout,
        #[cfg(feature = "tokio-transport")]
        shutdown_flush_timeout: self.shutdown_flush_timeout,
      };
      let handle = crate::std::endpoint_transport_bridge::EndpointTransportBridge::spawn(config)
        .map_err(|error| RemotingError::TransportUnavailable(format!("{error:?}")))?;
      *bridge_guard = Some(handle);
    }
    #[cfg(not(feature = "tokio-transport"))]
    {
      let _ = control;
    }
    Ok(())
  }
}
