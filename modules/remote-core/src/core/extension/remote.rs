//! Default core implementation of the remoting lifecycle API.

use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};
use core::mem;

use bytes::Bytes;
use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::{CorrelationId, RemotingLifecycleEvent},
};

use crate::core::{
  address::{Address, UniqueAddress},
  association::{Association, AssociationEffect, QuarantineReason},
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
  extension::{
    EventPublisher, RemoteEvent, RemoteEventReceiver, RemoteRunFuture, RemotingError, RemotingLifecycleState,
  },
  instrument::{NoopInstrument, RemoteInstrument},
  transport::{BackpressureSignal, RemoteTransport, TransportEndpoint, TransportError},
  wire::{
    AckCodec, Codec, ControlCodec, ControlPdu, EnvelopeCodec, EnvelopePdu, HandshakeCodec, HandshakePdu, HandshakeReq,
    HandshakeRsp, KIND_ACK, KIND_CONTROL, KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP,
  },
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
  inbound_envelopes:    Vec<InboundEnvelope>,
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
      inbound_envelopes: Vec::new(),
    }
  }

  /// Replaces the current instrument.
  ///
  /// Do not call this while [`Remote::run`] is being polled; the run loop uses
  /// the installed instrument for event handling.
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
  #[must_use]
  pub const fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a mut self, receiver: &'a mut S) -> RemoteRunFuture<'a, S> {
    RemoteRunFuture::new(self, receiver)
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

  /// Consumes buffered inbound envelopes observed by the core event loop.
  #[must_use]
  pub fn drain_inbound_envelopes(&mut self) -> Vec<InboundEnvelope> {
    mem::take(&mut self.inbound_envelopes)
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

  /// Handles one remote event.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when transport delivery
  /// fails, or [`RemotingError::UnimplementedEvent`] for event kinds whose
  /// concrete handling is not wired yet.
  pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<(), RemotingError> {
    match event {
      | RemoteEvent::TransportShutdown => {
        self.lifecycle.transition_to_shutdown_requested();
        Ok(())
      },
      | RemoteEvent::OutboundEnqueued { authority, envelope, now_ms } => {
        self.handle_outbound_enqueued(&authority, envelope, now_ms)?;
        Ok(())
      },
      | RemoteEvent::HandshakeTimerFired { authority, generation, now_ms } => {
        self.handle_handshake_timer_fired(&authority, generation, now_ms)
      },
      | RemoteEvent::InboundFrameReceived { authority, frame, now_ms } => {
        self.handle_inbound_frame_received(&authority, frame, now_ms)
      },
      | RemoteEvent::ConnectionLost { authority, cause, now_ms } => {
        self.handle_connection_lost(&authority, &cause, now_ms)
      },
    }
  }

  /// Returns `true` when the event loop should terminate.
  #[must_use]
  pub const fn is_terminated(&self) -> bool {
    self.lifecycle.is_terminated() || self.lifecycle.is_shutdown_requested()
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
    let prev_len = self.associations[association_index].total_outbound_len();
    let effects = self.associations[association_index].enqueue(*envelope, now_ms, self.instrument.as_mut());
    let curr_len = self.associations[association_index].total_outbound_len();
    self.apply_high_watermark_if_crossed(association_index, prev_len, curr_len, now_ms);
    self.apply_association_effects(association_index, effects, now_ms)?;
    if should_start_handshake {
      let effects = {
        let association = &mut self.associations[association_index];
        association.associate(authority.clone(), now_ms, &mut *self.instrument)
      };
      self.apply_association_effects(association_index, effects, now_ms)?;
    }
    self.drain_outbound(association_index, now_ms)
  }

  fn ensure_association(&mut self, remote: Address) -> Result<usize, RemotingError> {
    if let Some(index) = self.association_index_for_remote(&remote) {
      return Ok(index);
    }
    let local = self.local_unique_address_for(&remote).ok_or(RemotingError::TransportUnavailable)?;
    let association = Association::from_config(local, remote, &self.config);
    self.insert_association(association);
    Ok(self.associations.len() - 1)
  }

  fn ensure_association_for_handshake_request(&mut self, request: &HandshakeReq) -> Option<usize> {
    if !self.is_local_handshake_destination(request.to()) {
      return None;
    }
    if let Some(index) = self.association_index_for_remote(request.from().address()) {
      return Some(index);
    }
    let association = Association::from_config(
      UniqueAddress::new(request.to().clone(), 0),
      request.from().address().clone(),
      &self.config,
    );
    self.insert_association(association);
    Some(self.associations.len() - 1)
  }

  fn association_index_for_remote(&self, remote: &Address) -> Option<usize> {
    self.associations.iter().position(|association| association.remote() == remote)
  }

  fn association_index_for_authority(&self, authority: &TransportEndpoint) -> Option<usize> {
    let remote = parse_authority(authority.authority())?;
    self.association_index_for_remote(&remote)
  }

  fn handle_handshake_timer_fired(
    &mut self,
    authority: &TransportEndpoint,
    generation: u64,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_authority(authority) else {
      return Ok(());
    };
    if self.associations[association_index].handshake_generation() != generation {
      return Ok(());
    }
    let effects = {
      let association = &mut self.associations[association_index];
      association.handshake_timed_out(now_ms, None, &mut *self.instrument)
    };
    self.apply_association_effects(association_index, effects, now_ms)
  }

  fn handle_inbound_frame_received(
    &mut self,
    authority: &TransportEndpoint,
    frame: Vec<u8>,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    let kind = frame_kind(&frame)?;
    let mut bytes = Bytes::from(frame);
    match kind {
      | KIND_ENVELOPE => {
        let pdu = EnvelopeCodec::new().decode(&mut bytes).map_err(|_| RemotingError::CodecFailed)?;
        self.handle_inbound_envelope_pdu(authority, &pdu, now_ms)
      },
      | KIND_HANDSHAKE_REQ | KIND_HANDSHAKE_RSP => {
        let pdu = HandshakeCodec::new().decode(&mut bytes).map_err(|_| RemotingError::CodecFailed)?;
        self.handle_inbound_handshake_pdu(pdu, now_ms)
      },
      | KIND_CONTROL => {
        let pdu = ControlCodec::new().decode(&mut bytes).map_err(|_| RemotingError::CodecFailed)?;
        self.handle_inbound_control_pdu(&pdu, now_ms)
      },
      | KIND_ACK => {
        AckCodec::new().decode(&mut bytes).map_err(|_| RemotingError::CodecFailed)?;
        Ok(())
      },
      | _ => Err(RemotingError::CodecFailed),
    }
  }

  fn handle_inbound_handshake_pdu(&mut self, pdu: HandshakePdu, now_ms: u64) -> Result<(), RemotingError> {
    match pdu {
      | HandshakePdu::Req(request) => self.handle_inbound_handshake_request(&request, now_ms),
      | HandshakePdu::Rsp(response) => self.handle_inbound_handshake_response(&response, now_ms),
    }
  }

  fn handle_inbound_handshake_request(&mut self, request: &HandshakeReq, now_ms: u64) -> Result<(), RemotingError> {
    let Some(association_index) = self.ensure_association_for_handshake_request(request) else {
      return Ok(());
    };
    let accepted = {
      let association = &mut self.associations[association_index];
      match association.accept_handshake_request(request, now_ms, self.instrument.as_mut()) {
        | Ok(effects) => {
          let remote = association.remote().clone();
          let response = HandshakePdu::Rsp(HandshakeRsp::new(association.local().clone()));
          Some((remote, response, effects))
        },
        | Err(_err) => None,
      }
    };
    let Some((remote, response, effects)) = accepted else {
      return Ok(());
    };
    self.apply_association_effects(association_index, effects, now_ms)?;
    self.transport.send_handshake(&remote, response).map_err(|_| RemotingError::TransportUnavailable)?;
    self.drain_outbound(association_index, now_ms)
  }

  fn handle_inbound_handshake_response(&mut self, response: &HandshakeRsp, now_ms: u64) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_remote(response.from().address()) else {
      return Ok(());
    };
    let effects = {
      let association = &mut self.associations[association_index];
      match association.accept_handshake_response(response, now_ms, self.instrument.as_mut()) {
        | Ok(effects) => effects,
        | Err(_err) => return Ok(()),
      }
    };
    self.apply_association_effects(association_index, effects, now_ms)?;
    self.drain_outbound(association_index, now_ms)
  }

  fn handle_inbound_envelope_pdu(
    &mut self,
    authority: &TransportEndpoint,
    pdu: &EnvelopePdu,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_authority(authority) else {
      return Ok(());
    };
    let Some(remote_node) = self.associations[association_index].active_remote_node().cloned() else {
      return Ok(());
    };
    let recipient = ActorPathParser::parse(pdu.recipient_path()).map_err(|_| RemotingError::CodecFailed)?;
    let sender = match pdu.sender_path() {
      | Some(path) => Some(ActorPathParser::parse(path).map_err(|_| RemotingError::CodecFailed)?),
      | None => None,
    };
    let priority = OutboundPriority::from_wire(pdu.priority()).ok_or(RemotingError::CodecFailed)?;
    let envelope = InboundEnvelope::new(
      recipient,
      remote_node,
      AnyMessage::new(pdu.payload().clone()),
      sender,
      CorrelationId::new(pdu.correlation_hi(), pdu.correlation_lo()),
      priority,
    );
    self.associations[association_index].record_inbound(&envelope, now_ms, self.instrument.as_mut());
    self.inbound_envelopes.push(envelope);
    Ok(())
  }

  fn handle_inbound_control_pdu(&mut self, pdu: &ControlPdu, now_ms: u64) -> Result<(), RemotingError> {
    match pdu {
      | ControlPdu::Heartbeat { authority } | ControlPdu::HeartbeatResponse { authority, .. } => {
        self.record_control_activity(authority, now_ms);
        Ok(())
      },
      | ControlPdu::Quarantine { authority, reason } => {
        self.handle_inbound_quarantine_control(authority, reason, now_ms)
      },
      | ControlPdu::Shutdown { authority } => self.handle_inbound_shutdown_control(authority, now_ms),
    }
  }

  fn handle_connection_lost(
    &mut self,
    authority: &TransportEndpoint,
    cause: &TransportError,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    match cause {
      | TransportError::ConnectionClosed | TransportError::SendFailed => {},
      | TransportError::UnsupportedScheme
      | TransportError::NotAvailable
      | TransportError::AlreadyRunning
      | TransportError::NotStarted => return Err(RemotingError::TransportUnavailable),
    }
    let Some(association_index) = self.association_index_for_authority(authority) else {
      return Ok(());
    };
    let gate_effects = self.associations[association_index].gate(Some(now_ms), now_ms);
    self.apply_association_effects(association_index, gate_effects, now_ms)?;
    let recover_effects = {
      let association = &mut self.associations[association_index];
      association.recover(Some(authority.clone()), now_ms, self.instrument.as_mut())
    };
    self.apply_association_effects(association_index, recover_effects, now_ms)
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

  fn is_local_handshake_destination(&self, destination: &Address) -> bool {
    self.advertised_addresses.iter().any(|address| address == destination)
      || self.transport.addresses().iter().any(|address| address == destination)
      || self.transport.default_address().is_some_and(|address| address == destination)
  }

  fn record_control_activity(&mut self, authority: &str, now_ms: u64) {
    if let Some(remote) = parse_authority(authority)
      && let Some(index) = self.association_index_for_remote(&remote)
    {
      self.associations[index].record_handshake_activity(now_ms);
    }
  }

  fn handle_inbound_quarantine_control(
    &mut self,
    authority: &str,
    reason: &Option<String>,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(remote) = parse_authority(authority) else {
      return Ok(());
    };
    let Some(index) = self.association_index_for_remote(&remote) else {
      return Ok(());
    };
    let reason = QuarantineReason::new(reason.as_deref().unwrap_or("remote quarantine"));
    let effects = self.associations[index].quarantine(reason, now_ms, self.instrument.as_mut());
    self.apply_association_effects(index, effects, now_ms)
  }

  fn handle_inbound_shutdown_control(&mut self, authority: &str, now_ms: u64) -> Result<(), RemotingError> {
    let Some(remote) = parse_authority(authority) else {
      return Ok(());
    };
    let Some(index) = self.association_index_for_remote(&remote) else {
      return Ok(());
    };
    self.associations[index].record_handshake_activity(now_ms);
    let effects = self.associations[index].gate(None, now_ms);
    self.apply_association_effects(index, effects, now_ms)
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
            recursive.extend(self.associations[association_index].enqueue(envelope, now_ms, self.instrument.as_mut()));
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
    loop {
      let prev_len = self.associations[association_index].total_outbound_len();
      let was_user_paused = self.associations[association_index].send_queue().is_user_paused();
      let Some(envelope) = self.associations[association_index].next_outbound(now_ms, self.instrument.as_mut()) else {
        return Ok(());
      };
      if let Err((_err, envelope_for_retry)) = self.transport.send(envelope) {
        // 単一 envelope の送信失敗で event loop を終わらせると、他の peer 向け
        // association まで巻き添えで停止してしまう。`RemoteTransport::send` が失敗時に
        // 返してきた envelope を association に戻し、drain は中断するが、event loop は
        // 次の event を引き続き処理する。成功側のホットパスでは clone は発生しない。
        let effects =
          self.associations[association_index].enqueue(*envelope_for_retry, now_ms, self.instrument.as_mut());
        self.apply_association_effects(association_index, effects, now_ms)?;
        return Ok(());
      }
      let curr_len = self.associations[association_index].total_outbound_len();
      self.apply_low_watermark_if_crossed(association_index, prev_len, curr_len, was_user_paused, now_ms);
    }
  }

  fn apply_high_watermark_if_crossed(
    &mut self,
    association_index: usize,
    prev_len: usize,
    curr_len: usize,
    now_ms: u64,
  ) {
    let high = self.config.outbound_high_watermark();
    if prev_len <= high && curr_len > high {
      self.associations[association_index].apply_backpressure(
        BackpressureSignal::Apply,
        CorrelationId::nil(),
        now_ms,
        self.instrument.as_mut(),
      );
    }
  }

  fn apply_low_watermark_if_crossed(
    &mut self,
    association_index: usize,
    prev_len: usize,
    curr_len: usize,
    was_user_paused: bool,
    now_ms: u64,
  ) {
    let low = self.config.outbound_low_watermark();
    if was_user_paused && prev_len >= low && curr_len < low {
      self.associations[association_index].apply_backpressure(
        BackpressureSignal::Release,
        CorrelationId::nil(),
        now_ms,
        self.instrument.as_mut(),
      );
    }
  }
}

fn frame_kind(frame: &[u8]) -> Result<u8, RemotingError> {
  const KIND_OFFSET: usize = 5;
  frame.get(KIND_OFFSET).copied().ok_or(RemotingError::CodecFailed)
}

fn parse_authority(authority: &str) -> Option<Address> {
  let (system, endpoint) = authority.split_once('@')?;
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  let port = port.parse::<u16>().ok()?;
  Some(Address::new(system, host, port))
}

impl Remote {
  /// Starts the remote subsystem.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::AlreadyRunning`] if remoting is already running,
  /// or [`RemotingError::TransportUnavailable`] /
  /// [`RemotingError::InvalidTransition`] if the underlying transport could not
  /// be brought up.
  pub fn start(&mut self) -> Result<(), RemotingError> {
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

  /// Shuts the remote subsystem down.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting was never running.
  pub fn shutdown(&mut self) -> Result<(), RemotingError> {
    if self.lifecycle.is_terminated() {
      return Ok(());
    }
    if !self.lifecycle.is_shutdown_requested() {
      self.lifecycle.transition_to_shutdown()?;
    }
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

  /// Quarantines the given remote authority.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting is not currently
  /// running, or [`RemotingError::TransportUnavailable`] if the quarantine
  /// signal could not be propagated through the transport.
  pub fn quarantine(
    &mut self,
    address: &Address,
    uid: Option<u64>,
    reason: QuarantineReason,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    self.transport.quarantine(address, uid, reason).map_err(|_| RemotingError::TransportUnavailable)
  }

  /// Returns the local addresses this remoting instance advertises.
  #[must_use]
  pub fn addresses(&self) -> &[Address] {
    &self.advertised_addresses
  }
}
