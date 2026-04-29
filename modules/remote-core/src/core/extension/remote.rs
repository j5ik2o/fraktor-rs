//! Default core implementation of the remoting lifecycle API.

use alloc::{boxed::Box, string::ToString, vec::Vec};

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};

use crate::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  extension::{EventPublisher, Remoting, RemotingError, RemotingLifecycleState},
  transport::RemoteTransport,
};

/// Core remoting lifecycle implementation backed by a transport port.
///
/// `Remote` owns the core lifecycle state and talks to the outside world only
/// through [`RemoteTransport`]. Standard-library transports such as
/// `TcpRemoteTransport` are supplied by adapter crates and hidden behind the
/// port boundary.
pub struct Remote {
  lifecycle:            RemotingLifecycleState,
  transport:            Box<dyn RemoteTransport + Send>,
  config:               RemoteConfig,
  event_publisher:      EventPublisher,
  advertised_addresses: Vec<Address>,
}

impl Remote {
  /// Creates a new remote lifecycle instance.
  #[must_use]
  pub fn new<T>(transport: T, config: RemoteConfig, event_publisher: EventPublisher) -> Self
  where
    T: RemoteTransport + Send + 'static, {
    Self {
      lifecycle: RemotingLifecycleState::new(),
      transport: Box::new(transport),
      config,
      event_publisher,
      advertised_addresses: Vec::new(),
    }
  }

  /// Returns the current lifecycle state snapshot.
  #[must_use]
  pub const fn lifecycle(&self) -> &RemotingLifecycleState {
    &self.lifecycle
  }

  /// Returns the remote configuration used by this instance.
  #[must_use]
  pub const fn config(&self) -> &RemoteConfig {
    &self.config
  }

  fn publish_listen_started(&self) {
    for address in &self.advertised_addresses {
      self.event_publisher.publish_lifecycle(RemotingLifecycleEvent::ListenStarted {
        authority:      address.to_string(),
        // start と listen address の相関管理はまだ導入していないため nil 固定にする。
        correlation_id: CorrelationId::nil(),
      });
    }
  }
}

impl Remoting for Remote {
  fn start(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_start()?;
    let advertised_addresses = match self.transport.start() {
      | Ok(()) => self.transport.addresses().to_vec(),
      | Err(_) => {
        self.lifecycle.mark_start_failed()?;
        return Err(RemotingError::TransportUnavailable);
      },
    };
    self.advertised_addresses = advertised_addresses;
    self.lifecycle.mark_started()?;
    self.publish_listen_started();
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_shutdown()?;
    if self.lifecycle.is_terminated() {
      return Ok(());
    }
    self.transport.shutdown().map_err(|_| RemotingError::TransportUnavailable)?;
    self.lifecycle.mark_shutdown()?;
    self.advertised_addresses.clear();
    Ok(())
  }

  fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    self.transport.quarantine(address, uid, reason).map_err(|_| RemotingError::TransportUnavailable)
  }

  fn addresses(&self) -> &[Address] {
    &self.advertised_addresses
  }
}
