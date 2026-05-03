//! Default core implementation of the remoting lifecycle API.

use alloc::{boxed::Box, string::ToString, vec::Vec};

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};

use crate::core::{
  address::{Address, UniqueAddress},
  association::{Association, AssociationEffect, QuarantineReason},
  config::RemoteConfig,
  envelope::OutboundEnvelope,
  extension::{EventPublisher, RemoteEvent, RemoteEventReceiver, Remoting, RemotingError, RemotingLifecycleState},
  instrument::{NoopInstrument, RemoteInstrument},
  transport::{RemoteTransport, TransportEndpoint},
  wire::{HandshakePdu, HandshakeReq},
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
  associations:         Vec<Association>,
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
      associations: Vec::new(),
    }
  }

  /// Replaces the current instrument.
  ///
  /// [`Remote::run`] consumes `self`, so the instrument must be installed before
  /// starting the event loop.
  pub fn set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>) {
    self.instrument = instrument;
  }

  /// Runs the core remote event loop until shutdown is requested.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when transport delivery
  /// fails, or [`RemotingError::UnimplementedEvent`] for event kinds whose
  /// concrete handling is not wired yet. Returns
  /// [`RemotingError::EventReceiverClosed`] when the event source closes before
  /// [`RemoteEvent::TransportShutdown`] is observed.
  pub async fn run<S: RemoteEventReceiver>(mut self, receiver: &mut S) -> Result<(), RemotingError> {
    loop {
      let Some(event) = receiver.recv().await else {
        return Err(RemotingError::EventReceiverClosed);
      };
      if self.handle_remote_event(event)? {
        return Ok(());
      }
    }
  }

  /// Registers an association that the core event loop can drive.
  pub(crate) fn insert_association(&mut self, association: Association) {
    self.associations.push(association);
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

  fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError> {
    match event {
      | RemoteEvent::TransportShutdown => Ok(true),
      | RemoteEvent::OutboundEnqueued { authority, envelope, now_ms } => {
        self.handle_outbound_enqueued(&authority, envelope, now_ms)?;
        Ok(false)
      },
      | RemoteEvent::InboundFrameReceived { .. }
      | RemoteEvent::HandshakeTimerFired { .. }
      | RemoteEvent::ConnectionLost { .. } => Err(RemotingError::UnimplementedEvent),
    }
  }

  fn handle_outbound_enqueued(
    &mut self,
    authority: &TransportEndpoint,
    envelope: Box<OutboundEnvelope>,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    let remote = parse_authority(authority.authority()).ok_or(RemotingError::TransportUnavailable)?;
    let association_index = self.ensure_association(remote)?;
    let should_start_handshake = self.associations[association_index].state().is_idle();
    let effects = self.associations[association_index].enqueue(*envelope, now_ms);
    self.apply_association_effects(association_index, effects, now_ms)?;
    if should_start_handshake {
      let effects = self.associations[association_index].associate(authority.clone(), now_ms);
      self.apply_association_effects(association_index, effects, now_ms)?;
    }
    self.drain_outbound(association_index, now_ms)
  }

  fn ensure_association(&mut self, remote: Address) -> Result<usize, RemotingError> {
    if let Some(index) = self.associations.iter().position(|association| association.remote() == &remote) {
      return Ok(index);
    }
    let local = self.local_unique_address_for(&remote).ok_or(RemotingError::TransportUnavailable)?;
    let association = Association::from_config(local, remote, &self.config);
    self.insert_association(association);
    Ok(self.associations.len() - 1)
  }

  fn local_unique_address_for(&self, remote: &Address) -> Option<UniqueAddress> {
    self
      .transport
      .local_address_for_remote(remote)
      .or_else(|| self.transport.default_address())
      .or_else(|| self.advertised_addresses.first())
      .cloned()
      .map(|address| UniqueAddress::new(address, 0))
  }

  fn apply_association_effects(
    &mut self,
    association_index: usize,
    effects: Vec<AssociationEffect>,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let mut pending = effects;
    pending.reverse();
    while let Some(effect) = pending.pop() {
      match effect {
        | AssociationEffect::SendEnvelopes { envelopes } => {
          let mut recursive = Vec::new();
          for envelope in envelopes {
            recursive.extend(self.associations[association_index].enqueue(envelope, now_ms));
          }
          pending.extend(recursive.into_iter().rev());
        },
        | AssociationEffect::DiscardEnvelopes { .. } => {},
        | AssociationEffect::PublishLifecycle(event) => self.event_publisher.publish_lifecycle(event),
        | AssociationEffect::StartHandshake { authority, timeout, generation } => {
          let (remote, request) = {
            let association = &self.associations[association_index];
            (
              association.remote().clone(),
              HandshakePdu::Req(HandshakeReq::new(association.local().clone(), association.remote().clone())),
            )
          };
          self.transport.send_handshake(&remote, request).map_err(|_| RemotingError::TransportUnavailable)?;
          self
            .transport
            .schedule_handshake_timeout(&authority, timeout, generation)
            .map_err(|_| RemotingError::TransportUnavailable)?;
        },
      }
    }
    Ok(())
  }

  fn drain_outbound(&mut self, association_index: usize, now_ms: u64) -> Result<(), RemotingError> {
    while let Some(envelope) =
      self.associations[association_index].next_outbound_with_instrument(now_ms, self.instrument.as_mut())
    {
      let envelope_for_retry = envelope.clone();
      if self.transport.send(envelope).is_err() {
        let effects = self.associations[association_index].enqueue(envelope_for_retry, now_ms);
        self.apply_association_effects(association_index, effects, now_ms)?;
        return Err(RemotingError::TransportUnavailable);
      }
    }
    Ok(())
  }
}

fn parse_authority(authority: &str) -> Option<Address> {
  let (system, endpoint) = authority.split_once('@')?;
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  let port = port.parse::<u16>().ok()?;
  Some(Address::new(system, host, port))
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
