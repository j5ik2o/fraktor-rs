//! `StdRemoting` aggregate implementing the core `Remoting` trait.

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};
use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  extension::{EventPublisher, Remoting, RemotingError, RemotingLifecycleState},
  transport::RemoteTransport,
};
use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::std::{
  association_runtime::AssociationRegistry, tcp_transport::TcpRemoteTransport, watcher_actor::WatcherActorHandle,
};

/// `std + tokio` implementation of [`Remoting`].
///
/// `StdRemoting` is the Phase B replacement for the legacy
/// `RemotingControlHandle`. It owns:
///
/// - the [`TcpRemoteTransport`] wrapped in [`SharedLock`] so the association runtime tasks can
///   share it,
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
  transport:            SharedLock<TcpRemoteTransport>,
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
    transport: SharedLock<TcpRemoteTransport>,
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
  pub fn transport(&self) -> SharedLock<TcpRemoteTransport> {
    self.transport.clone()
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

  fn publish_listen_started(&self) {
    for address in &self.advertised_addresses {
      self.event_publisher.publish_lifecycle(RemotingLifecycleEvent::ListenStarted {
        authority:      address.to_string(),
        // Phase 1A では start と listen address の相関管理が未導入のため nil 固定にする。
        correlation_id: CorrelationId::nil(),
      });
    }
  }
}

impl Remoting for StdRemoting {
  fn start(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_start()?;
    let advertised_addresses = match self.transport.with_lock(|transport| {
      transport.start().map_err(|_| RemotingError::TransportUnavailable)?;
      Ok(transport.addresses().to_vec())
    }) {
      | Ok(addresses) => addresses,
      | Err(error) => {
        // transport.start() 失敗後に Starting に残ると再試行も shutdown もできなくなるため戻す。
        if let Err(rollback_error) = self.lifecycle.mark_start_failed() {
          tracing::error!(?rollback_error, "lifecycle rollback failed after transport start failure");
        }
        return Err(error);
      },
    };
    self.advertised_addresses = advertised_addresses;
    self.lifecycle.mark_started()?;
    self.publish_listen_started();
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_shutdown()?;
    self.transport.with_lock(|transport| {
      // Shutdown は best-effort とし、失敗は遷移と紐付けて観測できるよう警告に残す。
      if let Err(err) = transport.shutdown() {
        tracing::warn!(?err, "transport shutdown failed during StdRemoting::shutdown");
      }
    });
    if !self.lifecycle.is_terminated() {
      self.lifecycle.mark_shutdown()?;
    }
    Ok(())
  }

  fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    self.transport.with_lock(|transport| {
      transport.quarantine(address, uid, reason).map_err(|_| RemotingError::TransportUnavailable)
    })
  }

  fn addresses(&self) -> &[Address] {
    &self.advertised_addresses
  }
}
