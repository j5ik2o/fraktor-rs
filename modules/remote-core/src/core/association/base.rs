//! Per-remote association state machine.

use alloc::{
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::mem;

use fraktor_actor_core_rs::core::kernel::event::stream::{CorrelationId, RemotingLifecycleEvent};

use crate::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{
    association_effect::AssociationEffect, association_state::AssociationState,
    handshake_rejected_state::HandshakeRejectedState, handshake_validation_error::HandshakeValidationError,
    quarantine_reason::QuarantineReason, send_queue::SendQueue,
  },
  envelope::OutboundEnvelope,
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
  state:      AssociationState,
  send_queue: SendQueue,
  deferred:   Vec<OutboundEnvelope>,
  local:      UniqueAddress,
  remote:     Address,
}

impl Association {
  /// Creates a new [`Association`] in the [`AssociationState::Idle`] state.
  #[must_use]
  pub fn new(local: UniqueAddress, remote: Address) -> Self {
    Self { state: AssociationState::Idle, send_queue: SendQueue::new(), deferred: Vec::new(), local, remote }
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

  /// Returns the last monotonic millis at which handshake activity was observed.
  #[must_use]
  pub const fn last_used_at(&self) -> Option<u64> {
    match &self.state {
      | AssociationState::Active { last_used_at, .. } => Some(*last_used_at),
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

  // -------------------------------------------------------------------------
  // state transitions
  // -------------------------------------------------------------------------

  /// Starts handshake against the given endpoint. Valid only from
  /// [`AssociationState::Idle`].
  pub fn associate(&mut self, endpoint: TransportEndpoint, now_ms: u64) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Idle => {
        let effect = AssociationEffect::StartHandshake { endpoint: endpoint.clone() };
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
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    self.ensure_local_destination(request.to())?;
    self.ensure_remote_origin(request.from().address())?;
    self.handshake_accepted(remote_node_id_from_unique_address(request.from()), now_ms)
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
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    self.ensure_remote_origin(response.from().address())?;
    self.handshake_accepted(remote_node_id_from_unique_address(response.from()), now_ms)
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
  ) -> Result<Vec<AssociationEffect>, HandshakeValidationError> {
    if let AssociationState::Active { remote_node: current, last_used_at, .. } = &mut self.state
      && current == &remote_node
    {
      *last_used_at = now_ms;
      return Ok(Vec::new());
    }

    match &self.state {
      | AssociationState::Handshaking { .. } => {
        let mut effects = Vec::new();
        let deferred = mem::take(&mut self.deferred);
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
  pub fn handshake_timed_out(&mut self, _now_ms: u64, resume_at_ms: Option<u64>) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Handshaking { .. } => {
        let mut effects = Vec::new();
        let deferred = mem::take(&mut self.deferred);
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
  pub fn quarantine(&mut self, reason: QuarantineReason, _now_ms: u64) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Active { .. }
      | AssociationState::Handshaking { .. }
      | AssociationState::Gated { .. }
      | AssociationState::Idle => {
        let mut effects = Vec::new();
        let mut discarded = mem::take(&mut self.deferred);
        discarded.append(&mut self.send_queue.drain_all());
        effects.push(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined {
          authority:      self.authority_string(),
          reason:         reason.message().into(),
          correlation_id: CorrelationId::nil(),
        }));
        if !discarded.is_empty() {
          effects.push(AssociationEffect::DiscardEnvelopes { reason: reason.clone(), envelopes: discarded });
        }
        self.state = AssociationState::Quarantined { reason, resume_at: None };
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
  pub fn recover(&mut self, endpoint: Option<TransportEndpoint>, now_ms: u64) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Gated { .. } | AssociationState::Quarantined { .. } => match endpoint {
        | Some(endpoint) => {
          let effect = AssociationEffect::StartHandshake { endpoint: endpoint.clone() };
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
  pub fn enqueue(&mut self, envelope: OutboundEnvelope) -> Vec<AssociationEffect> {
    match &self.state {
      | AssociationState::Active { .. } => {
        let _ = self.send_queue.offer(envelope);
        Vec::new()
      },
      | AssociationState::Handshaking { .. } | AssociationState::Gated { .. } | AssociationState::Idle => {
        self.deferred.push(envelope);
        Vec::new()
      },
      | AssociationState::Quarantined { reason, .. } => {
        vec![AssociationEffect::DiscardEnvelopes { reason: reason.clone(), envelopes: vec![envelope] }]
      },
    }
  }

  /// Returns the next outbound envelope to send from the internal queue, or
  /// `None` if nothing is currently pending (or the user lane is paused and
  /// no system-priority traffic remains).
  pub fn next_outbound(&mut self) -> Option<OutboundEnvelope> {
    self.send_queue.next_outbound()
  }

  /// Applies a backpressure signal to the internal send queue.
  pub const fn apply_backpressure(&mut self, signal: BackpressureSignal) {
    self.send_queue.apply_backpressure(signal);
  }

  // -------------------------------------------------------------------------
  // helpers
  // -------------------------------------------------------------------------

  fn authority_string(&self) -> String {
    self.remote.to_string()
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
