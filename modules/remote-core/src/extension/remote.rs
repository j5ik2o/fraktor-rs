//! Default core implementation of the remoting lifecycle API.

#[cfg(test)]
#[path = "remote_test.rs"]
mod tests;

use alloc::{
  boxed::Box,
  collections::{BTreeMap, VecDeque},
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::{mem, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::{CorrelationId, RemotingLifecycleEvent},
  serialization::{SerializationExtensionShared, SerializedMessage, SerializerId},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};

use crate::{
  address::{Address, UniqueAddress},
  association::{Association, AssociationEffect, AssociationState, QuarantineReason},
  config::RemoteConfig,
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
  extension::{
    EventPublisher, RemoteDeploymentOutcome, RemoteDeploymentResponse, RemoteEvent, RemoteEventReceiver,
    RemoteFlushOutcome, RemoteFlushTimer, RemoteRunFuture, RemotingError, RemotingLifecycleState,
  },
  instrument::{NoopInstrument, RemoteInstrument},
  transport::{BackpressureSignal, RemoteTransport, TransportEndpoint, TransportError},
  watcher::{WatcherCommand, WatcherEffect, WatcherState},
  wire::{
    AckPdu, ControlPdu, EnvelopePdu, FlushScope, HandshakePdu, HandshakeReq, HandshakeRsp,
    RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentFailureCode, RemoteDeploymentPdu,
    WireFrame,
  },
};

const MAX_STALE_DEPLOYMENT_RESPONSES: usize = 128;

type DeploymentCorrelation = (u64, u32);
type PendingDeploymentResponses = BTreeMap<DeploymentCorrelation, PendingDeploymentResponse>;

struct PendingDeploymentResponse {
  authority:         Address,
  started_at_millis: u64,
}

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
  serialization:        ArcShared<SerializationExtensionShared>,
  instrument:           Box<dyn RemoteInstrument + Send>,
  advertised_addresses: Vec<Address>,
  explicit_peers:       Vec<Address>,
  associations:         Vec<Association>,
  inbound_envelopes:    Vec<InboundEnvelope>,
  watcher_state:        WatcherState,
  watcher_effects:      Vec<WatcherEffect>,
  deployment_pending:   PendingDeploymentResponses,
  deployment_stale:     VecDeque<RemoteDeploymentResponse>,
  deployment_outcomes:  Vec<RemoteDeploymentOutcome>,
  flush_outcomes:       Vec<RemoteFlushOutcome>,
}

fn accept_inbound_handshake_request(
  association: &mut Association,
  request: &HandshakeReq,
  now_ms: u64,
  instrument: &mut dyn RemoteInstrument,
  association_index: usize,
) -> Vec<AssociationEffect> {
  match association.accept_handshake_request(request, now_ms, instrument) {
    | Ok(effects) => effects,
    | Err(error) => {
      tracing::debug!(?error, association_index, ?request, now_ms, "accept handshake request failed");
      Default::default()
    },
  }
}

fn map_inbound_response_delivery_result(
  remote: &Address,
  operation: &'static str,
  result: Result<(), TransportError>,
) -> Result<bool, RemotingError> {
  match result {
    | Ok(()) => Ok(true),
    | Err(error @ (TransportError::Backpressure | TransportError::ConnectionClosed)) => {
      tracing::debug!(?error, remote = %remote, operation, "dropping inbound response because peer writer is unavailable");
      Ok(false)
    },
    | Err(
      error @ (TransportError::UnsupportedScheme
      | TransportError::NotAvailable
      | TransportError::AlreadyRunning
      | TransportError::NotStarted
      | TransportError::SendFailed),
    ) => {
      tracing::debug!(?error, remote = %remote, operation, "inbound response delivery failed");
      Err(RemotingError::TransportUnavailable)
    },
  }
}

impl Remote {
  /// Creates a new remote lifecycle instance.
  #[must_use]
  pub fn new<T>(
    transport: T,
    config: RemoteConfig,
    event_publisher: EventPublisher,
    serialization: ArcShared<SerializationExtensionShared>,
  ) -> Self
  where
    T: RemoteTransport + Send + 'static, {
    Self::with_instrument(transport, config, event_publisher, serialization, Box::new(NoopInstrument))
  }

  /// Creates a new remote lifecycle instance with a custom instrument.
  #[must_use]
  pub fn with_instrument<T>(
    transport: T,
    config: RemoteConfig,
    event_publisher: EventPublisher,
    serialization: ArcShared<SerializationExtensionShared>,
    instrument: Box<dyn RemoteInstrument + Send>,
  ) -> Self
  where
    T: RemoteTransport + Send + 'static, {
    Self {
      lifecycle: RemotingLifecycleState::new(),
      transport: Box::new(transport),
      config,
      event_publisher,
      serialization,
      instrument,
      advertised_addresses: Vec::new(),
      explicit_peers: Vec::new(),
      associations: Vec::new(),
      inbound_envelopes: Vec::new(),
      watcher_state: WatcherState::default(),
      watcher_effects: Vec::new(),
      deployment_pending: PendingDeploymentResponses::new(),
      deployment_stale: VecDeque::new(),
      deployment_outcomes: Vec::new(),
      flush_outcomes: Vec::new(),
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

  /// Consumes flush outcomes observed by the core event loop.
  #[must_use]
  pub fn drain_flush_outcomes(&mut self) -> Vec<RemoteFlushOutcome> {
    mem::take(&mut self.flush_outcomes)
  }

  /// Applies a watcher command to the core-owned watcher state.
  pub fn handle_watcher_command(&mut self, command: WatcherCommand) {
    self.apply_watcher_command(command);
  }

  /// Consumes watcher effects emitted by the core watcher state.
  #[must_use]
  pub fn drain_watcher_effects(&mut self) -> Vec<WatcherEffect> {
    mem::take(&mut self.watcher_effects)
  }

  /// Registers an origin-side deployment request as pending.
  pub fn register_deployment_request(
    &mut self,
    correlation_hi: u64,
    correlation_lo: u32,
    authority: Address,
    started_at_millis: u64,
  ) {
    self
      .deployment_pending
      .insert((correlation_hi, correlation_lo), PendingDeploymentResponse { authority, started_at_millis });
  }

  /// Cancels an origin-side deployment request without completing it.
  pub fn cancel_deployment_request(&mut self, correlation_hi: u64, correlation_lo: u32) {
    self.deployment_pending.remove(&(correlation_hi, correlation_lo));
  }

  /// Fails pending deployment requests for a terminated remote authority.
  pub fn fail_deployment_requests_for_terminated_authority(
    &mut self,
    authority: &str,
    reason: &str,
    observed_at_millis: u64,
  ) -> Vec<RemoteDeploymentResponse> {
    let Some(remote) = parse_authority(authority) else {
      tracing::warn!(authority, "remote deployment address termination authority is invalid");
      return Vec::new();
    };
    let keys = self
      .deployment_pending
      .iter()
      .filter_map(|(key, pending)| {
        let matches_authority = pending.authority == remote;
        let not_replayed_old_event = observed_at_millis >= pending.started_at_millis;
        if matches_authority && not_replayed_old_event { Some(*key) } else { None }
      })
      .collect::<Vec<_>>();
    let mut responses = Vec::with_capacity(keys.len());
    for (correlation_hi, correlation_lo) in keys {
      self.deployment_pending.remove(&(correlation_hi, correlation_lo));
      let failure = RemoteDeploymentCreateFailure::new(
        correlation_hi,
        correlation_lo,
        RemoteDeploymentFailureCode::AddressTerminated,
        deployment_address_terminated_failure_reason(authority, reason),
      );
      responses.push(RemoteDeploymentResponse::Failure(failure));
    }
    responses
  }

  /// Sends a create failure response for an adapter-side request delivery failure.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::TransportUnavailable`] when the failure response
  /// cannot be delivered through the configured transport.
  pub fn reject_deployment_create_request(
    &mut self,
    authority: &TransportEndpoint,
    request: &RemoteDeploymentCreateRequest,
    reason: String,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(remote) = deployment_response_remote(authority, request) else {
      tracing::warn!(
        authority = authority.authority(),
        origin_node = request.origin_node(),
        "remote deployment request rejection response address is invalid"
      );
      return Ok(());
    };
    let pdu = RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
      request.correlation_hi(),
      request.correlation_lo(),
      RemoteDeploymentFailureCode::SpawnFailed,
      reason,
    ));
    self.handle_outbound_deployment(&remote, pdu, now_ms)
  }

  /// Consumes deployment outcomes emitted by core protocol handling.
  #[must_use]
  pub fn drain_deployment_outcomes(&mut self) -> Vec<RemoteDeploymentOutcome> {
    mem::take(&mut self.deployment_outcomes)
  }

  /// Starts a flush session for active associations.
  ///
  /// When `authority` is `Some`, only the matching active association is
  /// targeted. When it is `None`, every active association is targeted.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] when remoting is not running, or
  /// [`RemotingError::TransportUnavailable`] when draining prior outbound work
  /// requires an unavailable transport.
  pub fn start_flush(
    &mut self,
    authority: Option<&TransportEndpoint>,
    scope: FlushScope,
    lane_ids: &[u32],
    now_ms: u64,
  ) -> Result<Vec<RemoteFlushTimer>, RemotingError> {
    self.lifecycle.ensure_running()?;
    let association_indices = self.flush_association_indices(authority);
    let timeout = self.config.shutdown_flush_timeout();
    for &association_index in &association_indices {
      self.drain_outbound(association_index, now_ms)?;
    }
    let mut timers = Vec::new();
    let mut outcomes = Vec::new();
    for association_index in association_indices {
      let effects = self.associations[association_index].start_flush(scope, lane_ids, timeout, now_ms);
      self.collect_flush_start_effects(association_index, effects, &mut timers, &mut outcomes);
    }
    self.flush_outcomes.extend(outcomes);
    Ok(timers)
  }

  /// Returns the remote configuration used by this instance.
  #[must_use]
  pub const fn config(&self) -> &RemoteConfig {
    &self.config
  }

  /// Establishes a transport peer writer for `remote`.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] when remoting is not running, or
  /// [`RemotingError::TransportUnavailable`] when the transport cannot
  /// establish the peer.
  pub fn connect_peer(&mut self, remote: &Address) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    self.transport.connect_peer(remote).map_err(|_| RemotingError::TransportUnavailable)?;
    self.remember_explicit_peer(remote);
    Ok(())
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
      | RemoteEvent::OutboundControl { remote, pdu, now_ms } => self.handle_outbound_control(&remote, pdu, now_ms),
      | RemoteEvent::OutboundDeployment { remote, pdu, now_ms } => {
        self.handle_outbound_deployment(&remote, pdu, now_ms)
      },
      | RemoteEvent::RedeliveryTimerFired { authority, now_ms } => {
        self.handle_redelivery_timer_fired(&authority, now_ms)
      },
      | RemoteEvent::HandshakeTimerFired { authority, generation, now_ms } => {
        self.handle_handshake_timer_fired(&authority, generation, now_ms)
      },
      | RemoteEvent::FlushTimerFired { authority, flush_id, now_ms } => {
        self.handle_flush_timer_fired(&authority, flush_id, now_ms)
      },
      | RemoteEvent::InboundFrameReceived { authority, frame, now_ms } => {
        self.handle_inbound_frame_received(&authority, frame, now_ms)
      },
      | RemoteEvent::ConnectionLost { authority, cause, now_ms } => {
        self.handle_connection_lost(&authority, &cause, now_ms)
      },
    }
  }

  /// Returns `true` when the event loop should stop polling events.
  #[must_use]
  pub const fn should_stop_event_loop(&self) -> bool {
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
    if !self.can_use_peer_for_outbound(&remote) {
      tracing::warn!(
        remote = %remote,
        "dropping outbound envelope because remote peer is not allowed for automatic dialing"
      );
      self.instrument.record_dropped_envelope(authority, &envelope, now_ms);
      return Ok(());
    }
    let association_index = self.ensure_association(remote)?;
    let should_start_handshake = self.associations[association_index].state().is_idle();
    let should_recover_handshake = self.associations[association_index].state().is_gated();
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
    if should_recover_handshake {
      let effects = {
        let association = &mut self.associations[association_index];
        association.recover(Some(authority.clone()), now_ms, &mut *self.instrument)
      };
      self.apply_association_effects(association_index, effects, now_ms)?;
    }
    self.drain_outbound(association_index, now_ms)
  }

  fn handle_outbound_control(&mut self, remote: &Address, pdu: ControlPdu, _now_ms: u64) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    if !self.can_use_peer_for_outbound(remote) {
      tracing::warn!(
        remote = %remote,
        "dropping outbound control frame because remote peer is not allowed for automatic dialing"
      );
      return Ok(());
    }
    self.transport.connect_peer(remote).map_err(|_| RemotingError::TransportUnavailable)?;
    map_wire_delivery_result(remote, self.transport.send_control(remote, pdu))
  }

  fn handle_outbound_deployment(
    &mut self,
    remote: &Address,
    pdu: RemoteDeploymentPdu,
    _now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    if !self.can_use_peer_for_outbound(remote) {
      tracing::warn!(
        remote = %remote,
        "dropping outbound deployment frame because remote peer is not allowed for automatic dialing"
      );
      return Ok(());
    }
    self.transport.connect_peer(remote).map_err(|_| RemotingError::TransportUnavailable)?;
    map_wire_delivery_result(remote, self.transport.send_deployment(remote, pdu))
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

  fn association_index_for_handshake_request(&self, request: &HandshakeReq) -> Option<usize> {
    if !self.is_local_handshake_destination(request.to()) {
      return None;
    }
    self.association_index_for_remote(request.from().address())
  }

  fn association_index_for_remote(&self, remote: &Address) -> Option<usize> {
    self.associations.iter().position(|association| association.remote() == remote)
  }

  fn remember_explicit_peer(&mut self, remote: &Address) {
    if !self.explicit_peers.iter().any(|peer| peer == remote) {
      self.explicit_peers.push(remote.clone());
    }
  }

  /// Returns whether `remote` has been explicitly connected through
  /// [`Remote::connect_peer`].
  #[must_use]
  pub fn is_explicit_peer(&self, remote: &Address) -> bool {
    self.explicit_peers.iter().any(|peer| peer == remote)
  }

  fn can_use_peer_for_outbound(&self, remote: &Address) -> bool {
    self.config.is_remote_peer_allowed(remote)
      || self.association_index_for_remote(remote).is_some()
      || self.is_explicit_peer(remote)
  }

  fn association_index_for_authority(&self, authority: &TransportEndpoint) -> Option<usize> {
    if let Some(remote) = parse_authority(authority.authority()) {
      return self.association_index_for_remote(&remote);
    }
    let (host, port) = parse_endpoint(authority.authority())?;
    self
      .associations
      .iter()
      .position(|association| association.remote().host() == host && association.remote().port() == port)
  }

  fn flush_association_indices(&self, authority: Option<&TransportEndpoint>) -> Vec<usize> {
    match authority {
      | Some(authority) => self
        .association_index_for_authority(authority)
        .filter(|index| self.associations[*index].state().is_active())
        .into_iter()
        .collect(),
      | None => self
        .associations
        .iter()
        .enumerate()
        .filter_map(|(index, association)| association.state().is_active().then_some(index))
        .collect(),
    }
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

  fn handle_redelivery_timer_fired(&mut self, authority: &TransportEndpoint, now_ms: u64) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_authority(authority) else {
      return Ok(());
    };
    if !self.associations[association_index].state().is_active() {
      return Ok(());
    }
    let resend_after_ms = duration_to_saturated_millis(self.config.system_message_resend_interval());
    let effects = self.associations[association_index].resend_due(now_ms, resend_after_ms);
    self.apply_association_effects(association_index, effects, now_ms)
  }

  fn handle_flush_timer_fired(
    &mut self,
    authority: &TransportEndpoint,
    flush_id: u64,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_authority(authority) else {
      return Ok(());
    };
    let effects = self.associations[association_index].flush_timed_out(flush_id, now_ms);
    self.apply_association_effects(association_index, effects, now_ms)
  }

  fn handle_inbound_frame_received(
    &mut self,
    authority: &TransportEndpoint,
    frame: WireFrame,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    match frame {
      | WireFrame::Envelope(pdu) => self.handle_inbound_envelope_pdu(authority, &pdu, now_ms),
      | WireFrame::Handshake(pdu) => self.handle_inbound_handshake_pdu(pdu, now_ms),
      | WireFrame::Control(pdu) => self.handle_inbound_control_pdu(authority, &pdu, now_ms),
      | WireFrame::Ack(pdu) => self.handle_inbound_ack_pdu(authority, &pdu, now_ms),
      | WireFrame::Deployment(pdu) => {
        self.handle_inbound_deployment_pdu(authority, pdu, now_ms);
        Ok(())
      },
    }
  }

  fn handle_inbound_handshake_pdu(&mut self, pdu: HandshakePdu, now_ms: u64) -> Result<(), RemotingError> {
    match pdu {
      | HandshakePdu::Req(request) => self.handle_inbound_handshake_request(&request, now_ms),
      | HandshakePdu::Rsp(response) => self.handle_inbound_handshake_response(&response, now_ms),
    }
  }

  fn handle_inbound_handshake_request(&mut self, request: &HandshakeReq, now_ms: u64) -> Result<(), RemotingError> {
    let Some(association_index) = self.association_index_for_handshake_request(request) else {
      return Ok(());
    };
    let (remote, response) = {
      let association = &self.associations[association_index];
      if !matches!(association.state(), AssociationState::Handshaking { .. } | AssociationState::Active { .. }) {
        tracing::debug!(association_index, remote = %association.remote(), "accept handshake request failed");
        return Ok(());
      }
      if association.local().address() != request.to() {
        tracing::debug!(association_index, remote = %association.remote(), "accept handshake request failed");
        return Ok(());
      }
      let remote = association.remote().clone();
      let response = HandshakePdu::Rsp(HandshakeRsp::new(association.local().clone()));
      (remote, response)
    };
    if !map_inbound_response_delivery_result(&remote, "connect_peer", self.transport.connect_peer(&remote))? {
      return Ok(());
    }
    if !map_inbound_response_delivery_result(
      &remote,
      "send_handshake",
      self.transport.send_handshake(&remote, response),
    )? {
      return Ok(());
    }
    let effects = accept_inbound_handshake_request(
      &mut self.associations[association_index],
      request,
      now_ms,
      self.instrument.as_mut(),
      association_index,
    );
    self.apply_association_effects(association_index, effects, now_ms)?;
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
        | Err(error) => {
          tracing::debug!(
            ?error,
            association_index,
            remote = %association.remote(),
            "accept handshake response failed"
          );
          return Ok(());
        },
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
    if priority.is_system() {
      let sequence = pdu.redelivery_sequence().ok_or(RemotingError::CodecFailed)?;
      let (should_deliver, effects) = {
        let association = &mut self.associations[association_index];
        association.observe_inbound_system_envelope(sequence, now_ms)
      };
      self.apply_association_effects(association_index, effects, now_ms)?;
      if !should_deliver {
        return Ok(());
      }
    }
    let serialized = SerializedMessage::new(
      SerializerId::from_raw(pdu.serializer_id()),
      pdu.manifest().map(ToString::to_string),
      pdu.payload().to_vec(),
    );
    let payload = match self.serialization.with_read(|serialization| serialization.deserialize(&serialized, None)) {
      | Ok(payload) => payload,
      | Err(error) => {
        tracing::debug!(?error, "inbound payload deserialization failed");
        return Ok(());
      },
    };
    let envelope = InboundEnvelope::new(
      recipient,
      remote_node,
      AnyMessage::from_erased(ArcShared::from_boxed(payload), None, false, false),
      sender,
      CorrelationId::new(pdu.correlation_hi(), pdu.correlation_lo()),
      priority,
    );
    self.buffer_inbound_envelope(association_index, envelope, now_ms);
    Ok(())
  }

  fn buffer_inbound_envelope(&mut self, association_index: usize, envelope: InboundEnvelope, now_ms: u64) {
    let limit = self.config.system_message_buffer_size();
    if self.inbound_envelopes.len() >= limit {
      tracing::warn!(
        remote = %self.associations[association_index].remote(),
        buffered = self.inbound_envelopes.len(),
        limit,
        correlation_id_hi = envelope.correlation_id().hi(),
        correlation_id_lo = envelope.correlation_id().lo(),
        priority = envelope.priority().to_wire(),
        "dropping inbound envelope because inbound delivery buffer is full"
      );
      return;
    }
    self.associations[association_index].record_inbound(&envelope, now_ms, self.instrument.as_mut());
    self.inbound_envelopes.push(envelope);
  }

  fn handle_inbound_control_pdu(
    &mut self,
    peer_authority: &TransportEndpoint,
    pdu: &ControlPdu,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    match pdu {
      | ControlPdu::Heartbeat { authority } => {
        self.handle_inbound_heartbeat_control(peer_authority, authority, now_ms);
        Ok(())
      },
      | ControlPdu::HeartbeatResponse { authority, .. } => {
        if let Some(index) = self.verified_control_association_index(peer_authority, authority) {
          self.associations[index].record_handshake_activity(now_ms);
          let remote = self.associations[index].remote().clone();
          let ControlPdu::HeartbeatResponse { uid, .. } = pdu else {
            return Ok(());
          };
          self.apply_watcher_command(WatcherCommand::HeartbeatResponseReceived {
            from: remote,
            uid:  *uid,
            now:  now_ms,
          });
        }
        Ok(())
      },
      | ControlPdu::Quarantine { authority, reason } => {
        self.handle_inbound_quarantine_control(peer_authority, authority, reason, now_ms)
      },
      | ControlPdu::Shutdown { authority } => self.handle_inbound_shutdown_control(peer_authority, authority, now_ms),
      | ControlPdu::FlushRequest { authority, flush_id, lane_id, expected_acks, .. } => self
        .handle_inbound_flush_request_control(peer_authority, authority, *flush_id, *lane_id, *expected_acks, now_ms),
      | ControlPdu::FlushAck { authority, flush_id, lane_id, expected_acks } => {
        self.handle_inbound_flush_ack_control(peer_authority, authority, *flush_id, *lane_id, *expected_acks, now_ms)
      },
      | ControlPdu::CompressionAdvertisement { .. } | ControlPdu::CompressionAck { .. } => Ok(()),
    }
  }

  fn handle_inbound_deployment_pdu(&mut self, authority: &TransportEndpoint, pdu: RemoteDeploymentPdu, now_ms: u64) {
    match pdu {
      | RemoteDeploymentPdu::CreateRequest(request) => {
        if !deployment_request_matches_authority(authority, &request) {
          tracing::warn!("dropping remote deployment request with mismatched origin authority");
          return;
        }
        let Some(response_remote) = parse_authority(authority.authority()) else {
          tracing::warn!(authority = authority.authority(), "remote deployment request authority is invalid");
          return;
        };
        self.deployment_outcomes.push(RemoteDeploymentOutcome::CreateRequested {
          response_remote,
          authority: authority.clone(),
          request: Box::new(request),
          now_ms,
        });
      },
      | RemoteDeploymentPdu::CreateSuccess(success) => {
        self.match_deployment_response(authority, RemoteDeploymentResponse::Success(success));
      },
      | RemoteDeploymentPdu::CreateFailure(failure) => {
        self.match_deployment_response(authority, RemoteDeploymentResponse::Failure(failure));
      },
    }
  }

  fn handle_inbound_heartbeat_control(&mut self, peer_authority: &TransportEndpoint, authority: &str, now_ms: u64) {
    let Some(index) = self.verified_control_association_index(peer_authority, authority) else {
      return;
    };
    self.associations[index].record_handshake_activity(now_ms);
    let remote = self.associations[index].remote().clone();
    self.apply_watcher_command(WatcherCommand::HeartbeatReceived { from: remote.clone(), now: now_ms });
    let local = self.associations[index].local().clone();
    let response = ControlPdu::HeartbeatResponse { authority: local.address().to_string(), uid: local.uid() };
    if let Err(error) = self.transport.send_control(&remote, response) {
      tracing::debug!(
        ?error,
        remote = %remote,
        "dropping heartbeat response because control channel is unavailable"
      );
    }
  }

  fn handle_inbound_flush_request_control(
    &mut self,
    peer_authority: &TransportEndpoint,
    authority: &str,
    flush_id: u64,
    lane_id: u32,
    expected_acks: u32,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(index) = self.verified_control_association_index(peer_authority, authority) else {
      return Ok(());
    };
    self.associations[index].record_handshake_activity(now_ms);
    let remote = self.associations[index].remote().clone();
    let local = self.associations[index].local().clone();
    let response = ControlPdu::FlushAck { authority: local.address().to_string(), flush_id, lane_id, expected_acks };
    map_wire_delivery_result(&remote, self.transport.send_control(&remote, response))
  }

  fn handle_inbound_flush_ack_control(
    &mut self,
    peer_authority: &TransportEndpoint,
    authority: &str,
    flush_id: u64,
    lane_id: u32,
    expected_acks: u32,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(index) = self.verified_control_association_index(peer_authority, authority) else {
      return Ok(());
    };
    self.associations[index].record_handshake_activity(now_ms);
    let effects = self.associations[index].apply_flush_ack(flush_id, lane_id, expected_acks);
    self.apply_association_effects(index, effects, now_ms)
  }

  fn verified_control_association_index(
    &self,
    peer_authority: &TransportEndpoint,
    claimed_authority: &str,
  ) -> Option<usize> {
    let claimed_remote = parse_authority(claimed_authority)?;
    let index = self.association_index_for_authority(peer_authority)?;
    let peer_remote = self.associations[index].remote();
    if peer_remote != &claimed_remote {
      let peer = peer_authority.authority();
      let associated = peer_remote.to_string();
      tracing::warn!(
        "ignoring control pdu with mismatched authority: peer={peer}, claimed={claimed_authority}, associated={associated}"
      );
      return None;
    }
    Some(index)
  }

  fn apply_watcher_command(&mut self, command: WatcherCommand) {
    self.watcher_effects.extend(self.watcher_state.handle(command));
  }

  fn match_deployment_response(&mut self, authority: &TransportEndpoint, response: RemoteDeploymentResponse) {
    let Some(remote) = parse_authority(authority.authority()) else {
      self.record_stale_deployment_response(response);
      return;
    };
    let key = (response.correlation_hi(), response.correlation_lo());
    let Some(pending) = self.deployment_pending.get(&key) else {
      self.record_stale_deployment_response(response);
      return;
    };
    if pending.authority != remote {
      self.record_stale_deployment_response(response);
      return;
    }
    self.deployment_pending.remove(&key);
    self.complete_deployment_response(response);
  }

  fn complete_deployment_response(&mut self, response: RemoteDeploymentResponse) {
    self.deployment_outcomes.push(RemoteDeploymentOutcome::ResponseCompleted { response });
  }

  fn record_stale_deployment_response(&mut self, response: RemoteDeploymentResponse) {
    if self.deployment_stale.len() >= MAX_STALE_DEPLOYMENT_RESPONSES {
      self.deployment_stale.pop_front();
    }
    self.deployment_stale.push_back(response);
    tracing::warn!(
      stale_responses = self.deployment_stale.len(),
      "remote deployment response did not match a pending request"
    );
  }

  fn handle_inbound_ack_pdu(
    &mut self,
    authority: &TransportEndpoint,
    pdu: &AckPdu,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    tracing::debug!(
      authority = %authority.authority(),
      sequence_number = pdu.sequence_number(),
      cumulative_ack = pdu.cumulative_ack(),
      nack_bitmap = pdu.nack_bitmap(),
      now_ms,
      "inbound ack pdu observed"
    );
    if let Some(index) = self.association_index_for_authority(authority) {
      let effects = self.associations[index].apply_ack(pdu, now_ms);
      self.apply_association_effects(index, effects, now_ms)?;
    }
    Ok(())
  }

  fn handle_connection_lost(
    &mut self,
    authority: &TransportEndpoint,
    cause: &TransportError,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    match cause {
      | TransportError::ConnectionClosed | TransportError::SendFailed | TransportError::Backpressure => {},
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
      // `gate` で設定した deadline と `recover` 側の状態遷移に backoff 判定を委譲し、
      // connection lost のイベント処理自体は単一の再起動指示として完結させる。
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

  fn handle_inbound_quarantine_control(
    &mut self,
    peer_authority: &TransportEndpoint,
    authority: &str,
    reason: &Option<String>,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(index) = self.verified_control_association_index(peer_authority, authority) else {
      return Ok(());
    };
    let reason = QuarantineReason::new(reason.as_deref().unwrap_or("remote quarantine"));
    let effects = self.associations[index].quarantine(reason, now_ms, self.instrument.as_mut());
    self.apply_association_effects(index, effects, now_ms)
  }

  fn handle_inbound_shutdown_control(
    &mut self,
    peer_authority: &TransportEndpoint,
    authority: &str,
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    let Some(index) = self.verified_control_association_index(peer_authority, authority) else {
      return Ok(());
    };
    self.associations[index].record_handshake_activity(now_ms);
    let gate_effects = self.associations[index].gate(None, now_ms);
    self.apply_association_effects(index, gate_effects, now_ms)?;
    if self.associations[index].state().is_quarantined() {
      return Ok(());
    }
    let recover_effects = self.associations[index].recover(None, now_ms, self.instrument.as_mut());
    self.apply_association_effects(index, recover_effects, now_ms)
  }

  pub(crate) fn collect_flush_start_effects(
    &mut self,
    association_index: usize,
    effects: Vec<AssociationEffect>,
    timers: &mut Vec<RemoteFlushTimer>,
    outcomes: &mut Vec<RemoteFlushOutcome>,
  ) {
    let mut pending = effects;
    pending.reverse();
    while let Some(effect) = pending.pop() {
      match effect {
        | AssociationEffect::ScheduleFlushTimeout { authority, flush_id, deadline_ms, .. } => {
          if !timers.iter().any(|timer| timer.authority() == &authority && timer.flush_id() == flush_id) {
            timers.push(RemoteFlushTimer::new(authority, flush_id, deadline_ms));
          }
        },
        | AssociationEffect::SendFlushRequest { authority, flush_id, scope, lane_id, expected_acks } => {
          let (remote, local_authority) = {
            let association = &self.associations[association_index];
            (association.remote().clone(), association.local().address().to_string())
          };
          let pdu = ControlPdu::FlushRequest { authority: local_authority, flush_id, scope, lane_id, expected_acks };
          if let Err(error) = self.transport.send_flush_request(&remote, pdu, lane_id) {
            tracing::warn!(
              ?error,
              remote = %remote,
              flush_id,
              lane_id,
              "flush request delivery failed"
            );
            timers.retain(|timer| timer.authority() != &authority || timer.flush_id() != flush_id);
            pending.retain(|effect| {
              !matches!(effect, AssociationEffect::SendFlushRequest { flush_id: pending_flush_id, .. } if *pending_flush_id == flush_id)
            });
            let effects = self.associations[association_index]
              .fail_flush(flush_id, format!("flush request send failed: {error:?}"));
            pending.extend(effects.into_iter().rev());
          }
        },
        | AssociationEffect::FlushCompleted { authority, flush_id, scope } => {
          outcomes.push(RemoteFlushOutcome::Completed { authority, flush_id, scope });
        },
        | AssociationEffect::FlushTimedOut { authority, flush_id, scope, pending_lanes } => {
          outcomes.push(RemoteFlushOutcome::TimedOut { authority, flush_id, scope, pending_lanes });
        },
        | AssociationEffect::FlushFailed { authority, flush_id, scope, pending_lanes, reason } => {
          outcomes.push(RemoteFlushOutcome::Failed { authority, flush_id, scope, pending_lanes, reason });
        },
        | AssociationEffect::StartHandshake { .. }
        | AssociationEffect::SendEnvelopes { .. }
        | AssociationEffect::SendAck { .. }
        | AssociationEffect::ResendEnvelopes { .. }
        | AssociationEffect::DiscardEnvelopes { .. }
        | AssociationEffect::PublishLifecycle(_) => {},
      }
    }
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
        | AssociationEffect::DiscardEnvelopes { .. }
        | AssociationEffect::ScheduleFlushTimeout { .. }
        | AssociationEffect::SendFlushRequest { .. } => {},
        | AssociationEffect::SendAck { pdu } => {
          let remote = self.associations[association_index].remote().clone();
          map_wire_delivery_result(&remote, self.transport.send_ack(&remote, pdu))?;
        },
        | AssociationEffect::FlushCompleted { authority, flush_id, scope } => {
          self.flush_outcomes.push(RemoteFlushOutcome::Completed { authority, flush_id, scope });
        },
        | AssociationEffect::FlushTimedOut { authority, flush_id, scope, pending_lanes } => {
          self.flush_outcomes.push(RemoteFlushOutcome::TimedOut { authority, flush_id, scope, pending_lanes });
        },
        | AssociationEffect::FlushFailed { authority, flush_id, scope, pending_lanes, reason } => {
          self.flush_outcomes.push(RemoteFlushOutcome::Failed { authority, flush_id, scope, pending_lanes, reason });
        },
        | AssociationEffect::ResendEnvelopes { envelopes } => {
          for envelope in envelopes {
            self.associations[association_index].mark_system_envelope_sent(&envelope, now_ms);
            self.instrument.on_send(&envelope, now_ms);
            match self.transport.send(envelope) {
              | Ok(()) => {},
              | Err((TransportError::SendFailed, envelope)) => {
                let authority = TransportEndpoint::new(self.associations[association_index].remote().to_string());
                self.instrument.record_dropped_envelope(&authority, &envelope, now_ms);
              },
              | Err((_error, envelope)) => {
                let recursive =
                  self.associations[association_index].enqueue(*envelope, now_ms, self.instrument.as_mut());
                pending.extend(recursive.into_iter().rev());
              },
            }
          }
        },
        | AssociationEffect::PublishLifecycle(event) => self.event_publisher.publish_lifecycle(event),
        | AssociationEffect::StartHandshake { authority, timeout, generation } => {
          let (remote, request) = {
            let association = &self.associations[association_index];
            (
              association.remote().clone(),
              HandshakePdu::Req(HandshakeReq::new(association.local().clone(), association.remote().clone())),
            )
          };
          self.transport.connect_peer(&remote).map_err(|_| RemotingError::TransportUnavailable)?;
          map_wire_delivery_result(&remote, self.transport.send_handshake(&remote, request))?;
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
      match self.transport.send(envelope) {
        | Ok(()) => {},
        | Err((TransportError::SendFailed, envelope)) => {
          // 永久的な payload 送信失敗を再投入すると、次のイベントごとに同じ envelope が
          // 先頭で失敗し続ける。呼び出し元へ同期的に戻せないため、ログに残して蓄積を止める。
          let authority = TransportEndpoint::new(self.associations[association_index].remote().to_string());
          self.instrument.record_dropped_envelope(&authority, &envelope, now_ms);
          tracing::warn!(
            remote = %authority.authority(),
            correlation_id_hi = envelope.correlation_id().hi(),
            correlation_id_lo = envelope.correlation_id().lo(),
            priority = envelope.priority().to_wire(),
            "discarding outbound envelope after transport send failed"
          );
          return Ok(());
        },
        | Err((_err, envelope_for_retry)) => {
          // 単一 envelope の送信失敗で event loop を終わらせると、他の peer 向け
          // association まで巻き添えで停止してしまう。`RemoteTransport::send` が失敗時に
          // 返してきた envelope を association に戻し、drain は中断するが、event loop は
          // 次の event を引き続き処理する。成功側のホットパスでは clone は発生しない。
          let effects =
            self.associations[association_index].enqueue(*envelope_for_retry, now_ms, self.instrument.as_mut());
          self.apply_association_effects(association_index, effects, now_ms)?;
          return Ok(());
        },
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
        BackpressureSignal::Notify,
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

fn parse_authority(authority: &str) -> Option<Address> {
  let (system, endpoint) = authority.split_once('@')?;
  let (host, port) = parse_endpoint(endpoint)?;
  Some(Address::new(system, host, port))
}

fn deployment_request_matches_authority(
  authority: &TransportEndpoint,
  request: &RemoteDeploymentCreateRequest,
) -> bool {
  match (parse_authority(authority.authority()), parse_authority(request.origin_node())) {
    | (Some(authority), Some(origin)) => authority == origin,
    | _ => false,
  }
}

fn deployment_response_remote(
  authority: &TransportEndpoint,
  request: &RemoteDeploymentCreateRequest,
) -> Option<Address> {
  let raw_authority = authority.authority();
  let authority = parse_authority(authority.authority());
  let origin = parse_authority(request.origin_node());
  match (authority, origin) {
    | (Some(authority), Some(origin)) => {
      if authority != origin {
        tracing::warn!(
          authority = authority.to_string(),
          origin_node = origin.to_string(),
          "remote deployment origin node differs from inbound authority; replying to inbound authority"
        );
      }
      Some(authority)
    },
    | (Some(authority), None) => Some(authority),
    | (None, Some(origin)) => {
      tracing::warn!(
        authority = raw_authority,
        origin_node = origin.to_string(),
        "remote deployment inbound authority is invalid; falling back to origin node"
      );
      Some(origin)
    },
    | (None, None) => None,
  }
}

fn deployment_address_terminated_failure_reason(authority: &str, reason: &str) -> String {
  format!("remote deployment target address terminated: authority={authority}, reason={reason}")
}

fn parse_endpoint(endpoint: &str) -> Option<(&str, u16)> {
  let (host, port) = endpoint.rsplit_once(':')?;
  let host = host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host);
  Some((host, port.parse::<u16>().ok()?))
}

fn duration_to_saturated_millis(duration: Duration) -> u64 {
  let millis = duration.as_millis();
  if millis > u128::from(u64::MAX) { u64::MAX } else { millis as u64 }
}

fn map_wire_delivery_result(remote: &Address, result: Result<(), TransportError>) -> Result<(), RemotingError> {
  match result {
    | Ok(()) => Ok(()),
    | Err(error @ TransportError::Backpressure) => {
      tracing::warn!(
        ?error,
        remote = %remote,
        "wire frame delivery hit transport backpressure; keeping remote event loop alive"
      );
      Ok(())
    },
    | Err(
      TransportError::UnsupportedScheme
      | TransportError::NotAvailable
      | TransportError::AlreadyRunning
      | TransportError::NotStarted
      | TransportError::SendFailed
      | TransportError::ConnectionClosed,
    ) => Err(RemotingError::TransportUnavailable),
  }
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
  /// `now_ms` is the caller-provided monotonic millis used for local
  /// association deadlines and instrumentation.
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
    now_ms: u64,
  ) -> Result<(), RemotingError> {
    self.lifecycle.ensure_running()?;
    if let Some(index) = self.association_index_for_remote(address) {
      let effects = self.associations[index].quarantine(reason.clone(), now_ms, self.instrument.as_mut());
      self.apply_association_effects(index, effects, now_ms)?;
    }
    self.transport.quarantine(address, uid, reason).map_err(|_| RemotingError::TransportUnavailable)
  }

  /// Returns the local addresses this remoting instance advertises.
  #[must_use]
  pub fn addresses(&self) -> &[Address] {
    &self.advertised_addresses
  }
}
