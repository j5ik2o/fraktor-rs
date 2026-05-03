//! Default core implementation of the remoting lifecycle API.

use alloc::{boxed::Box, string::ToString, vec::Vec};

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};

use crate::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  extension::{EventPublisher, RemoteEvent, RemoteEventReceiver, Remoting, RemotingError, RemotingLifecycleState},
  instrument::{NoopInstrument, RemoteInstrument},
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
  instrument:           Box<dyn RemoteInstrument + Send>,
  advertised_addresses: Vec<Address>,
}

impl Remote {
  /// Creates a new remote lifecycle instance.
  #[must_use]
  pub fn new<T>(transport: T, config: RemoteConfig, event_publisher: EventPublisher) -> Self
  where
    T: RemoteTransport + Send + 'static, {
    Self::with_instrument(transport, config, event_publisher, Box::new(NoopInstrument))
  }

  /// Creates a new remote lifecycle instance with a custom instrument.
  #[must_use]
  pub fn with_instrument<T>(
    transport: T,
    config: RemoteConfig,
    event_publisher: EventPublisher,
    instrument: Box<dyn RemoteInstrument + Send>,
  ) -> Self
  where
    T: RemoteTransport + Send + 'static, {
    Self {
      lifecycle: RemotingLifecycleState::new(),
      transport: Box::new(transport),
      config,
      event_publisher,
      instrument,
      advertised_addresses: Vec::new(),
    }
  }

  /// Replaces the current instrument.
  ///
  /// [`Remote::run`] consumes `self`, so the instrument must be installed before
  /// starting the event loop.
  pub fn set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>) {
    self.instrument = instrument;
  }

  /// Runs the core remote event loop until the receiver closes or shutdown is requested.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when transport delivery
  /// fails, or [`RemotingError::UnimplementedEvent`] for event kinds whose
  /// concrete handling is not wired yet.
  pub async fn run<S: RemoteEventReceiver>(mut self, receiver: &mut S) -> Result<(), RemotingError> {
    while let Some(event) = receiver.recv().await {
      let transport = &mut *self.transport;
      let instrument = &mut *self.instrument;
      if handle_remote_event(transport, instrument, event)? {
        return Ok(());
      }
    }
    Ok(())
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

fn handle_remote_event(
  transport: &mut dyn RemoteTransport,
  instrument: &mut dyn RemoteInstrument,
  event: RemoteEvent,
) -> Result<bool, RemotingError> {
  match event {
    | RemoteEvent::TransportShutdown => Ok(true),
    | RemoteEvent::OutboundEnqueued { envelope, now_ms, .. } => {
      instrument.on_send(envelope.as_ref(), now_ms);
      transport.send(*envelope).map_err(|_| RemotingError::TransportUnavailable)?;
      Ok(false)
    },
    | RemoteEvent::InboundFrameReceived { .. }
    | RemoteEvent::HandshakeTimerFired { .. }
    | RemoteEvent::ConnectionLost { .. } => Err(RemotingError::UnimplementedEvent),
  }
}

impl Remoting for Remote {
  fn start(&mut self) -> Result<(), RemotingError> {
    self.lifecycle.transition_to_start()?;
    let advertised_addresses = match self.transport.start() {
      | Ok(()) => self.transport.addresses().to_vec(),
      | Err(_) => {
        match self.transport.shutdown() {
          | Ok(()) => {},
          | Err(_cleanup_error) => {
            // start 失敗後の cleanup 失敗は、元の起動失敗と同じ
            // `TransportUnavailable` として呼び出し元へ返す。
          },
        }
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
    if self.transport.shutdown().is_err() {
      self.lifecycle.mark_shutdown_failed()?;
      return Err(RemotingError::TransportUnavailable);
    }
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
