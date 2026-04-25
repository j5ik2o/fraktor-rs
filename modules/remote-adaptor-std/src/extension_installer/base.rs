//! `StdRemoting` aggregate implementing the core `Remoting` trait.

use std::sync::{Arc, Mutex};

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};
use fraktor_remote_core_rs::{
  address::Address,
  association::QuarantineReason,
  extension::{EventPublisher, Remoting, RemotingError, RemotingLifecycleState},
  transport::RemoteTransport,
};

use crate::{
  association_runtime::AssociationRegistry, tcp_transport::TcpRemoteTransport, watcher_actor::WatcherActorHandle,
};

/// `std + tokio` implementation of [`Remoting`].
///
/// `StdRemoting` is the Phase B replacement for the legacy
/// `RemotingControlHandle`. It owns:
///
/// - the [`TcpRemoteTransport`] (wrapped in `Arc<Mutex<...>>` so the association runtime tasks can
///   share it),
/// - an [`AssociationRegistry`] of per-remote `AssociationShared` handles,
/// - a [`WatcherActorHandle`] for submitting watch / unwatch / heartbeat commands to the watcher
///   actor task,
/// - a closed [`RemotingLifecycleState`] state machine.
///
/// The actual TCP runtime tasks (`run_outbound_loop`, `run_inbound_dispatch`,
/// `run_heartbeat_loop`) are spawned by the caller; `StdRemoting` only owns
/// their handles. This keeps `StdRemoting` runtime-agnostic — the same type
/// can be driven from `tokio::main` or from a manual runtime.
pub struct StdRemoting {
  lifecycle:            RemotingLifecycleState,
  transport:            Arc<Mutex<TcpRemoteTransport>>,
  registry:             AssociationRegistry,
  watcher:              Option<WatcherActorHandle>,
  event_publisher:      EventPublisher,
  advertised_addresses: Vec<Address>,
}

impl StdRemoting {
  /// Creates a new `StdRemoting` wrapping the given transport.
  ///
  /// The watcher handle is optional — callers that have not yet started a
  /// watcher actor can pass `None`. The handle can be installed later with
  /// [`StdRemoting::install_watcher`].
  #[must_use]
  pub fn new(
    transport: Arc<Mutex<TcpRemoteTransport>>,
    watcher: Option<WatcherActorHandle>,
    event_publisher: EventPublisher,
  ) -> Self {
    Self {
      lifecycle: RemotingLifecycleState::new(),
      transport,
      registry: AssociationRegistry::new(),
      watcher,
      event_publisher,
      advertised_addresses: Vec::new(),
    }
  }

  /// Installs (or replaces) the watcher handle.
  pub fn install_watcher(&mut self, watcher: WatcherActorHandle) {
    self.watcher = Some(watcher);
  }

  /// Returns a clone of the underlying transport handle.
  ///
  /// Exposed so the runtime tasks (`run_outbound_loop`, `run_inbound_dispatch`)
  /// can share the same transport instance.
  #[must_use]
  pub fn transport(&self) -> Arc<Mutex<TcpRemoteTransport>> {
    Arc::clone(&self.transport)
  }

  /// Returns an immutable reference to the association registry.
  #[must_use]
  pub const fn registry(&self) -> &AssociationRegistry {
    &self.registry
  }

  /// Returns a mutable reference to the association registry.
  pub const fn registry_mut(&mut self) -> &mut AssociationRegistry {
    &mut self.registry
  }

  /// Returns the optional watcher actor handle.
  #[must_use]
  pub const fn watcher(&self) -> Option<&WatcherActorHandle> {
    self.watcher.as_ref()
  }

  /// Returns the current lifecycle state snapshot.
  #[must_use]
  pub const fn lifecycle(&self) -> &RemotingLifecycleState {
    &self.lifecycle
  }
}

impl Remoting for StdRemoting {
  fn start(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_start()?;
    let advertised_addresses = {
      let mut transport = self.transport.lock().map_err(|_| RemotingError::TransportUnavailable)?;
      transport.start().map_err(|_| RemotingError::TransportUnavailable)?;
      transport.addresses().to_vec()
    };
    self.advertised_addresses = advertised_addresses;
    self.lifecycle.mark_started()?;
    self.publish_listen_started();
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_shutdown()?;
    {
      let mut transport = self.transport.lock().map_err(|_| RemotingError::TransportUnavailable)?;
      // Best-effort shutdown — record any failure as a tracing warning so
      // the operator can correlate it with the lifecycle transition, but
      // always reach the `Shutdown` terminal state regardless.
      if let Err(err) = transport.shutdown() {
        tracing::warn!(?err, "transport shutdown failed during StdRemoting::shutdown");
      }
    }
    self.lifecycle.mark_shutdown()?;
    Ok(())
  }

  fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    let mut transport = self.transport.lock().map_err(|_| RemotingError::TransportUnavailable)?;
    transport.quarantine(address, uid, reason).map_err(|_| RemotingError::TransportUnavailable)
  }

  fn addresses(&self) -> &[Address] {
    &self.advertised_addresses
  }
}

impl StdRemoting {
  fn publish_listen_started(&self) {
    for address in &self.advertised_addresses {
      self.event_publisher.publish_lifecycle(RemotingLifecycleEvent::ListenStarted {
        authority:      address.to_string(),
        correlation_id: CorrelationId::nil(),
      });
    }
  }
}
