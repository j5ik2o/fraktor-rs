//! Per-remote association state machine.

#[cfg(test)]
mod tests;

use alloc::{
  format,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{mem, time::Duration};

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};

use crate::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{
    association_effect::AssociationEffect, association_state::AssociationState,
    handshake_rejected_state::HandshakeRejectedState, handshake_validation_error::HandshakeValidationError,
    offer_outcome::OfferOutcome, quarantine_reason::QuarantineReason, send_queue::SendQueue,
  },
  config::{
    DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE, DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE,
    DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER, RemoteConfig,
  },
  envelope::{InboundEnvelope, OutboundEnvelope},
  instrument::{HandshakePhase, RemoteInstrument},
  transport::{BackpressureSignal, TransportEndpoint},
  wire::{HandshakeReq, HandshakeRsp},
};

/// Per-remote association aggregating the state machine, the send queue, and
/// the deferred buffer used while the peer is not yet reachable.
///
/// Pekko Artery's `Association` class (Scala, ~1240 lines) fans out across
/// `EndpointWriter`, `EndpointAssociation`, and `EndpointTransportBridge` in
/// the legacy code; Decision 4 re-unifies the responsibilities here.
#[derive(Debug)]
pub struct Association {
  state: AssociationState,
  send_queue: SendQueue,
  deferred: Vec<OutboundEnvelope>,
  deferred_system_count: usize,
  deferred_user_count: usize,
  outbound_control_queue_size: usize,
  outbound_message_queue_size: usize,
  remove_quarantined_association_after: Duration,
  handshake_timeout: Duration,
  handshake_generation: u64,
  local: UniqueAddress,
  remote: Address,
}

impl Association {
  /// Creates a new [`Association`] in the [`AssociationState::Idle`] state.
  #[must_use]
  pub fn new(local: UniqueAddress, remote: Address) -> Self {
    Self::with_limits(
      local,
      remote,
      DEFAULT_OUTBOUND_CONTROL_QUEUE_SIZE,
      DEFAULT_OUTBOUND_MESSAGE_QUEUE_SIZE,
      DEFAULT_REMOVE_QUARANTINED_ASSOCIATION_AFTER,
      DEFAULT_HANDSHAKE_TIMEOUT,
    )
  }

  /// Creates a new [`Association`] using queue limits from [`RemoteConfig`].
  #[must_use]
  pub fn from_config(local: UniqueAddress, remote: Address, config: &RemoteConfig) -> Self {
    Self::with_limits(
      local,
      remote,
      config.outbound_control_queue_size(),
      config.outbound_message_queue_size(),
      config.remove_quarantined_association_after(),
      config.handshake_timeout(),
    )
  }

  /// Returns the current state snapshot.
  #[must_use]
  pub const fn state(&self) -> &AssociationState {
    &self.state
  }

  /// Returns the local [`UniqueAddress`] this association belongs to.
  #[must_use]
  pub const fn local(&self) -> &UniqueAddress {
    &self.local
  }

  /// Returns the target remote [`Address`].
  #[must_use]
  pub const fn remote(&self) -> &Address {
    &self.remote
  }

  /// Returns the number of envelopes currently waiting in the deferred queue.
  #[must_use]
  pub const fn deferred_len(&self) -> usize {
    self.deferred.len()
  }

  /// Returns a reference to the underlying send queue.
  #[must_use]
  pub const fn send_queue(&self) -> &SendQueue {
    &self.send_queue
  }

  /// Returns the combined outbound queue length, excluding deferred envelopes.
  #[must_use]
  pub fn total_outbound_len(&self) -> usize {
    self.send_queue.len()
  }

  /// Records an inbound envelope observation through `instrument`.
  pub fn record_inbound(&self, envelope: &InboundEnvelope, now_ms: u64, instrument: &mut dyn RemoteInstrument) {
    instrument.on_receive(envelope, now_ms);
  }

  /// Returns the current handshake generation.
  #[must_use]
  pub const fn handshake_generation(&self) -> u64 {
    self.handshake_generation
  }

  /// Returns the last monotonic millis at which handshake activity was observed.
  #[must_use]
  pub const fn last_used_at(&self) -> Option<u64> {
    match &self.state {
      | AssociationState::Active { last_used_at, .. } => Some(*last_used_at),
      | _ => None,
    }
  }

  /// Returns the remote node identity learned through handshake, when active.
  #[must_use]
  pub const fn active_remote_node(&self) -> Option<&RemoteNodeId> {
    match &self.state {
      | AssociationState::Active { remote_node, .. } => Some(remote_node),
      | _ => None,
    }
  }

  /// Returns `true` when an active association has been idle for `interval_ms`.
  #[must_use]
  pub const fn is_liveness_probe_due(&self, now_ms: u64, interval_ms: u64) -> bool {
    match &self.state {
      | AssociationState::Active { last_used_at, .. } => now_ms.saturating_sub(*last_used_at) >= interval_ms,
      | _ => false,
    }
  }

  /// Returns `true` when a quarantined association reached its removal deadline.
  #[must_use]
  pub const fn is_quarantine_removal_due(&self, now_ms: u64) -> bool {
    match &self.state {
      | AssociationState::Quarantined { resume_at: Some(resume_at), .. } => now_ms >= *resume_at,
      | _ => false,
    }
  }

  // -------------------------------------------------------------------------
  // state transitions
  // -------------------------------------------------------------------------

  /// Starts handshake against the given endpoint. Valid only from
  /// [`AssociationState::Idle`].
  pub fn associate(
    &mut self,
    endpoint: TransportEndpoint,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Idle => {
        let generation = self.advance_handshake_generation();
        instrument.record_handshake(&endpoint, HandshakePhase::Started, now_ms);
        let effect = AssociationEffect::StartHandshake {
          authority: endpoint.clone(),
          timeout: self.handshake_timeout,
          generation,
        };
        self.state = AssociationState::Handshaking { endpoint, started_at: now_ms };
        vec![effect]
      },
      // Re-associating from any non-Idle state is a no-op here; the caller is
      // expected to route through `recover` or `quarantine` first.
      | _ => Vec::new(),
    }
  }

  /// Accepts a handshake request after verifying both the remote origin and the local destination.
  ///
  /// # Errors
  ///
  /// Returns [`HandshakeValidationError`] when the request does not belong to this association.
  pub fn accept_handshake_request(
    &mut self,
    request: &HandshakeReq,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    self.ensure_local_destination(request.to())?;
    self.ensure_remote_origin(request.from().address())?;
    self.handshake_accepted(remote_node_id_from_unique_address(request.from()), now_ms, instrument)
  }

  /// Accepts a handshake response after verifying the remote origin.
  ///
  /// # Errors
  ///
  /// Returns [`HandshakeValidationError`] when the response does not belong to
  /// this association, or when the association cannot transition into `Active`
  /// from its current state (`Idle`, `Gated`, `Quarantined`).
  pub fn accept_handshake_response(
    &mut self,
    response: &HandshakeRsp,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    self.ensure_remote_origin(response.from().address())?;
    self.handshake_accepted(remote_node_id_from_unique_address(response.from()), now_ms, instrument)
  }

  /// Transitions `Handshaking` → `Active`, flushing any deferred envelopes.
  ///
  /// Returns `Err(RejectedInState)` for `Idle` / `Gated` / `Quarantined`: those
  /// states must not be silently promoted to `Active` because the inbound
  /// dispatcher would otherwise reply with `HandshakeRsp` and the peer would
  /// observe an `Active` association while the local side stays unreachable.
  // Idle / Gated / Quarantined で `Ok(Vec::new())` を返すと、inbound dispatcher が
  // 「accept_handshake_request が Ok = HandshakeRsp を返してよい」という規約に従い
  // 応答 PDU を送信してしまう。リモートは Active と思い込むがローカルは引き続き
  // 到達不能のまま、という非対称なプロトコル状態を作るため、ここで明示的に拒否する。
  fn handshake_accepted(
    &mut self,
    remote_node: RemoteNodeId,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    if let AssociationState::Active { remote_node: current, last_used_at, .. } = &mut self.state
      && current == &remote_node
    {
      *last_used_at = now_ms;
      instrument.record_handshake(&self.authority_endpoint(), HandshakePhase::Accepted, now_ms);
      return Ok(Vec::new());
    }

    match &self.state {
      | AssociationState::Handshaking { .. } => {
        let mut effects = Vec::new();
        let deferred = mem::take(&mut self.deferred);
        self.clear_deferred_counts();
        instrument.record_handshake(&self.authority_endpoint(), HandshakePhase::Accepted, now_ms);
        effects.push(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected {
          authority:      self.authority_string(),
          remote_system:  remote_node.system().to_string(),
          remote_uid:     remote_node.uid(),
          correlation_id: CorrelationId::nil(),
        }));
        if !deferred.is_empty() {
          effects.push(AssociationEffect::SendEnvelopes { envelopes: deferred });
        }
        self.state = AssociationState::Active { remote_node, established_at: now_ms, last_used_at: now_ms };
        Ok(effects)
      },
      | AssociationState::Active { .. } => {
        instrument.record_handshake(&self.authority_endpoint(), HandshakePhase::Accepted, now_ms);
        let effect = AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected {
          authority:      self.authority_string(),
          remote_system:  remote_node.system().to_string(),
          remote_uid:     remote_node.uid(),
          correlation_id: CorrelationId::nil(),
        });
        self.state = AssociationState::Active { remote_node, established_at: now_ms, last_used_at: now_ms };
        Ok(vec![effect])
      },
      | AssociationState::Idle => {
        Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Idle })
      },
      | AssociationState::Gated { .. } => {
        Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Gated })
      },
      | AssociationState::Quarantined { .. } => {
        Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Quarantined })
      },
    }
  }

  /// Records handshake activity for an active association.
  pub const fn record_handshake_activity(&mut self, now_ms: u64) {
    if let AssociationState::Active { last_used_at, .. } = &mut self.state {
      *last_used_at = now_ms;
    }
  }

  /// Transitions `Handshaking` → `Gated`, discarding deferred envelopes via an
  /// effect and publishing a `Gated` lifecycle event.
  pub fn handshake_timed_out(
    &mut self,
    now_ms: u64,
    resume_at_ms: Option<u64>,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Handshaking { .. } => {
        let mut effects = Vec::new();
        let deferred = mem::take(&mut self.deferred);
        self.clear_deferred_counts();
        instrument.record_handshake(&self.authority_endpoint(), HandshakePhase::Rejected, now_ms);
        effects.push(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Gated {
          authority:      self.authority_string(),
          correlation_id: CorrelationId::nil(),
        }));
        if !deferred.is_empty() {
          effects.push(AssociationEffect::DiscardEnvelopes {
            reason:    QuarantineReason::new("handshake timed out"),
            envelopes: deferred,
          });
        }
        self.state = AssociationState::Gated { resume_at: resume_at_ms };
        effects
      },
      | _ => Vec::new(),
    }
  }

  /// Transitions any non-terminal state into `Quarantined`, discarding both
  /// deferred and send-queue contents.
  pub fn quarantine(
    &mut self,
    reason: QuarantineReason,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Active { .. }
      | AssociationState::Handshaking { .. }
      | AssociationState::Gated { .. }
      | AssociationState::Idle => {
        let mut effects = Vec::new();
        let mut discarded = mem::take(&mut self.deferred);
        self.clear_deferred_counts();
        discarded.append(&mut self.send_queue.drain_all());
        effects.push(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined {
          authority:      self.authority_string(),
          reason:         reason.message().into(),
          correlation_id: CorrelationId::nil(),
        }));
        instrument.record_quarantine(&self.authority_endpoint(), &reason, now_ms);
        if !discarded.is_empty() {
          effects.push(AssociationEffect::DiscardEnvelopes { reason: reason.clone(), envelopes: discarded });
        }
        self.state =
          AssociationState::Quarantined { reason, resume_at: Some(self.quarantine_removal_deadline(now_ms)) };
        effects
      },
      | AssociationState::Quarantined { .. } => Vec::new(),
    }
  }

  /// Transitions `Active` → `Gated` without a handshake round-trip. All other
  /// states are left untouched.
  pub fn gate(&mut self, resume_at_ms: Option<u64>, _now_ms: u64) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Active { .. } => {
        let effect = AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Gated {
          authority:      self.authority_string(),
          correlation_id: CorrelationId::nil(),
        });
        self.state = AssociationState::Gated { resume_at: resume_at_ms };
        vec![effect]
      },
      | _ => Vec::new(),
    }
  }

  /// Transitions out of `Gated` / `Quarantined`:
  ///
  /// - `Some(endpoint)` → `Handshaking`, emitting `StartHandshake`.
  /// - `None` → `Idle`, no effect emitted.
  ///
  /// Calls from `Idle`, `Handshaking`, or `Active` are no-ops.
  pub fn recover(
    &mut self,
    endpoint: Option<TransportEndpoint>,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Gated { .. } | AssociationState::Quarantined { .. } => match endpoint {
        | Some(endpoint) => {
          let generation = self.advance_handshake_generation();
          instrument.record_handshake(&endpoint, HandshakePhase::Started, now_ms);
          let effect = AssociationEffect::StartHandshake {
            authority: endpoint.clone(),
            timeout: self.handshake_timeout,
            generation,
          };
          self.state = AssociationState::Handshaking { endpoint, started_at: now_ms };
          vec![effect]
        },
        | None => {
          self.state = AssociationState::Idle;
          Vec::new()
        },
      },
      | _ => Vec::new(),
    }
  }

  // -------------------------------------------------------------------------
  // send path
  // -------------------------------------------------------------------------

  /// Enqueues an outbound envelope. Behaviour depends on the current state:
  ///
  /// - `Active` → push into the internal send queue.
  /// - `Handshaking` / `Gated` / `Idle` → push into the deferred buffer.
  /// - `Quarantined` → return a `DiscardEnvelopes` effect immediately.
  pub fn enqueue(
    &mut self,
    envelope: OutboundEnvelope,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Active { .. } => self.enqueue_active(envelope, now_ms, instrument),
      | AssociationState::Handshaking { .. } | AssociationState::Gated { .. } | AssociationState::Idle => {
        self.enqueue_deferred(envelope)
      },
      | AssociationState::Quarantined { reason, .. } => {
        vec![AssociationEffect::DiscardEnvelopes { reason: reason.clone(), envelopes: vec![envelope] }]
      },
    }
  }

  /// Returns the next outbound envelope to send from the internal queue, or
  /// `None` if nothing is currently pending (or the user lane is paused and
  /// no system-priority traffic remains).
  pub fn next_outbound(&mut self, now_ms: u64, instrument: &mut dyn RemoteInstrument) -> Option<OutboundEnvelope> {
    let envelope = self.send_queue.next_outbound();
    if let Some(envelope) = &envelope {
      instrument.on_send(envelope, now_ms);
    }
    envelope
  }

  /// Applies a backpressure signal to the internal send queue.
  pub fn apply_backpressure(
    &mut self,
    signal: BackpressureSignal,
    correlation_id: CorrelationId,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) {
    instrument.record_backpressure(&self.authority_endpoint(), signal, correlation_id, now_ms);
    self.send_queue.apply_backpressure(signal);
  }

  // -------------------------------------------------------------------------
  // helpers
  // -------------------------------------------------------------------------

  fn with_limits(
    local: UniqueAddress,
    remote: Address,
    outbound_control_queue_size: usize,
    outbound_message_queue_size: usize,
    remove_quarantined_association_after: Duration,
    handshake_timeout: Duration,
  ) -> Self {
    Self {
      state: AssociationState::Idle,
      send_queue: SendQueue::with_limits(outbound_control_queue_size, outbound_message_queue_size),
      deferred: Vec::new(),
      deferred_system_count: 0,
      deferred_user_count: 0,
      outbound_control_queue_size,
      outbound_message_queue_size,
      remove_quarantined_association_after,
      handshake_timeout,
      handshake_generation: 0,
      local,
      remote,
    }
  }

  const fn advance_handshake_generation(&mut self) -> u64 {
    self.handshake_generation = self.handshake_generation.wrapping_add(1);
    self.handshake_generation
  }

  fn enqueue_active(
    &mut self,
    envelope: OutboundEnvelope,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    match self.send_queue.offer(envelope) {
      | OfferOutcome::Accepted => Vec::new(),
      | OfferOutcome::QueueFull { envelope } if envelope.priority().is_system() => {
        self.control_queue_overflow_effects(*envelope, now_ms, instrument)
      },
      | OfferOutcome::QueueFull { envelope } => Self::queue_full_discard_effect(*envelope),
    }
  }

  fn enqueue_deferred(&mut self, envelope: OutboundEnvelope) -> Vec<AssociationEffect> {
    if self.deferred_has_capacity_for(&envelope) {
      self.increment_deferred_count(&envelope);
      self.deferred.push(envelope);
      Vec::new()
    } else {
      Self::queue_full_discard_effect(envelope)
    }
  }

  const fn deferred_has_capacity_for(&self, envelope: &OutboundEnvelope) -> bool {
    if envelope.priority().is_system() {
      self.deferred_system_count < self.outbound_control_queue_size
    } else {
      self.deferred_user_count < self.outbound_message_queue_size
    }
  }

  const fn increment_deferred_count(&mut self, envelope: &OutboundEnvelope) {
    if envelope.priority().is_system() {
      self.deferred_system_count += 1;
    } else {
      self.deferred_user_count += 1;
    }
  }

  const fn clear_deferred_counts(&mut self) {
    self.deferred_system_count = 0;
    self.deferred_user_count = 0;
  }

  fn queue_full_discard_effect(envelope: OutboundEnvelope) -> Vec<AssociationEffect> {
    vec![AssociationEffect::DiscardEnvelopes {
      reason:    QuarantineReason::new("outbound queue overflow"),
      envelopes: vec![envelope],
    }]
  }

  fn control_queue_overflow_effects(
    &mut self,
    envelope: OutboundEnvelope,
    now_ms: u64,
    instrument: &mut dyn RemoteInstrument,
  ) -> Vec<AssociationEffect> {
    let reason =
      QuarantineReason::new(format!("Due to overflow of control queue, size [{}]", self.outbound_control_queue_size));
    let mut effects = self.quarantine(reason.clone(), now_ms, instrument);
    Self::append_discarded_envelope(&mut effects, reason, envelope);
    effects
  }

  fn append_discarded_envelope(
    effects: &mut Vec<AssociationEffect>,
    reason: QuarantineReason,
    envelope: OutboundEnvelope,
  ) {
    for effect in effects.iter_mut() {
      if let AssociationEffect::DiscardEnvelopes { envelopes, .. } = effect {
        envelopes.push(envelope);
        return;
      }
    }
    effects.push(AssociationEffect::DiscardEnvelopes { reason, envelopes: vec![envelope] });
  }

  fn quarantine_removal_deadline(&self, now_ms: u64) -> u64 {
    now_ms.saturating_add(duration_to_non_zero_millis(self.remove_quarantined_association_after))
  }

  fn authority_string(&self) -> String {
    self.remote.to_string()
  }

  fn authority_endpoint(&self) -> TransportEndpoint {
    TransportEndpoint::new(self.authority_string())
  }

  fn ensure_local_destination(&self, actual: &Address) -> Result<(), HandshakeValidationError> {
    if self.local.address() == actual {
      Ok(())
    } else {
      Err(HandshakeValidationError::UnexpectedDestination {
        expected: self.local.address().clone(),
        actual:   actual.clone(),
      })
    }
  }

  fn ensure_remote_origin(&self, actual: &Address) -> Result<(), HandshakeValidationError> {
    if &self.remote == actual {
      Ok(())
    } else {
      Err(HandshakeValidationError::UnexpectedRemote { expected: self.remote.clone(), actual: actual.clone() })
    }
  }
}

fn remote_node_id_from_unique_address(address: &UniqueAddress) -> RemoteNodeId {
  RemoteNodeId::new(address.address().system(), address.address().host(), Some(address.address().port()), address.uid())
}

fn duration_to_non_zero_millis(duration: Duration) -> u64 {
  let millis = duration.as_millis();
  if millis == 0 {
    1
  } else if millis > u128::from(u64::MAX) {
    u64::MAX
  } else {
    millis as u64
  }
}
