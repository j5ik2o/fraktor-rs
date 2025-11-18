//! Concrete [`RemotingControl`] handle shared with runtime components.

#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_actor_rs::core::{
  actor_prim::{actor_path::ActorPathParts, actor_ref::ActorRefGeneric},
  event_stream::BackpressureSignal,
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::{
  RemotingBackpressureListener, RemotingConnectionSnapshot, RemotingControl, RemotingError, RemotingExtensionConfig,
  core::{
    endpoint_manager::RemoteNodeId,
    event_publisher::EventPublisher,
    flight_recorder::{CorrelationTrace, CorrelationTraceHop, RemotingFlightRecorder, RemotingMetric},
    transport::{RemoteTransport, TransportFactory},
  },
};

const DEFAULT_RECORDER_CAPACITY: usize = 64;

struct RemotingControlShared<TB: RuntimeToolbox + 'static> {
  _system:         ActorSystemGeneric<TB>,
  publisher:       ArcShared<EventPublisher<TB>>,
  supervisor:      ToolboxMutex<Option<ActorRefGeneric<TB>>, TB>,
  listeners:       ToolboxMutex<Vec<ArcShared<dyn RemotingBackpressureListener>>, TB>,
  transport:       ToolboxMutex<Option<ArcShared<dyn RemoteTransport<TB>>>, TB>,
  flight_recorder: RemotingFlightRecorder,
  config:          RemotingExtensionConfig,
  started:         AtomicBool,
  shutdown:        AtomicBool,
}

/// Shared handle implementing the [`RemotingControl`] interface.
pub struct RemotingControlHandle<TB: RuntimeToolbox + 'static> {
  shared: ArcShared<RemotingControlShared<TB>>,
}

impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  /// Creates a new handle bound to the provided actor system.
  #[must_use]
  pub(crate) fn new(system: &ActorSystemGeneric<TB>, config: RemotingExtensionConfig) -> Self {
    let event_stream = system.event_stream();
    let publisher = ArcShared::new(EventPublisher::new(event_stream));
    let flight_recorder = RemotingFlightRecorder::new(DEFAULT_RECORDER_CAPACITY);
    let shared = RemotingControlShared {
      _system: system.clone(),
      publisher,
      supervisor: <TB::MutexFamily as SyncMutexFamily>::create(None),
      listeners: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      transport: <TB::MutexFamily as SyncMutexFamily>::create(None),
      flight_recorder,
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

  pub(crate) fn publish_shutdown(&self) {
    if !self.shared.shutdown.swap(true, Ordering::SeqCst) {
      self.publisher().lifecycle_shutdown();
    }
  }

  fn notify_backpressure_internal(&self, signal: BackpressureSignal, authority: &str) {
    let publisher = self.publisher();
    let correlation_id = publisher.next_correlation_id();
    publisher.backpressure(authority.to_string(), signal, correlation_id);
    self.shared.flight_recorder.record_trace(CorrelationTrace::new(
      correlation_id,
      authority.to_string(),
      CorrelationTraceHop::Send,
    ));
    self.shared.flight_recorder.record_metric(RemotingMetric::new(authority).with_backpressure(Some(signal)));
    let listeners = self.shared.listeners.lock().clone();
    for listener in listeners.iter() {
      listener.on_signal(signal, authority);
    }
  }

  fn publisher(&self) -> ArcShared<EventPublisher<TB>> {
    self.shared.publisher.clone()
  }

  pub(crate) fn flight_recorder(&self) -> RemotingFlightRecorder {
    self.shared.flight_recorder.clone()
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
        let publisher = self.publisher();
        publisher.lifecycle_starting();
        let correlation = publisher.next_correlation_id();
        publisher.lifecycle_listen_started(canonical_authority(&self.shared.config), correlation);
        Ok(())
      },
      | Err(error) => {
        self.shared.started.store(false, Ordering::SeqCst);
        self.publisher().lifecycle_error(format!("{error}"));
        Err(error)
      },
    }
  }

  fn associate(&self, address: &ActorPathParts) -> Result<(), RemotingError> {
    if !self.shared.started.load(Ordering::SeqCst) {
      return Err(RemotingError::NotStarted);
    }
    let (authority, host, port) = parse_authority(address)?;
    let remote = RemoteNodeId::new(address.system().to_string(), host, port, 0);
    let state = self.shared._system.state();
    let _ = state.remote_authority_set_connected(&authority);
    let publisher = self.publisher();
    let correlation = publisher.next_correlation_id();
    publisher.lifecycle_connected(authority.clone(), &remote, correlation);
    self.shared.flight_recorder.record_metric(RemotingMetric::new(authority.clone()));
    self.refresh_snapshot();
    Ok(())
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
    self.publisher().lifecycle_shutdown();
    Ok(())
  }

  fn register_backpressure_listener(&self, listener: ArcShared<dyn RemotingBackpressureListener>) {
    self.shared.listeners.lock().push(listener);
  }

  fn connections_snapshot(&self) -> Vec<RemotingConnectionSnapshot> {
    self.refresh_snapshot();
    self.shared.flight_recorder.endpoint_snapshot()
  }
}

impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  #[allow(dead_code)]
  pub(crate) fn notify_backpressure(&self, signal: BackpressureSignal, authority: &str) {
    self.notify_backpressure_internal(signal, authority);
  }
}

fn canonical_authority(config: &RemotingExtensionConfig) -> String {
  let host = config.remoting().canonical_host();
  match config.remoting().canonical_port() {
    | Some(port) => format!("{host}:{port}"),
    | None => host.to_string(),
  }
}

fn parse_authority(parts: &ActorPathParts) -> Result<(String, String, Option<u16>), RemotingError> {
  let endpoint = parts.authority_endpoint().ok_or_else(|| RemotingError::message("authority missing"))?;
  let mut split = endpoint.splitn(2, ':');
  let host = split.next().unwrap_or_default().to_string();
  let port = split.next().and_then(|value| value.parse::<u16>().ok());
  Ok((endpoint, host, port))
}

impl<TB: RuntimeToolbox + 'static> RemotingControlHandle<TB> {
  fn refresh_snapshot(&self) {
    let state = self.shared._system.state();
    let snapshot: Vec<RemotingConnectionSnapshot> = state
      .remote_authority_snapshots()
      .into_iter()
      .map(|(authority, status)| RemotingConnectionSnapshot::new(authority, status))
      .collect();
    self.shared.flight_recorder.update_endpoint_snapshot(snapshot);
  }
}
