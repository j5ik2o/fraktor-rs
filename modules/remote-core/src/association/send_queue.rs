//! Dual-priority queue owned by an [`crate::association::Association`].

use alloc::{collections::VecDeque, vec::Vec};
use core::mem;

use crate::{
  association::offer_outcome::OfferOutcome,
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::BackpressureSignal,
};

/// Default capacity hint for each priority lane.
const DEFAULT_CAPACITY: usize = 16;

/// Dual-priority queue used by [`crate::association::Association`] to buffer
/// outbound envelopes.
///
/// System-priority envelopes are always drained before user-priority ones.
/// User-priority traffic can be paused by [`SendQueue::apply_backpressure`] so that
/// downstream consumers (the transport layer) can exert flow control without
/// starving system signalling.
#[derive(Debug)]
pub struct SendQueue {
  system:      VecDeque<OutboundEnvelope>,
  user:        VecDeque<OutboundEnvelope>,
  user_paused: bool,
}

impl SendQueue {
  /// Creates a new, empty [`SendQueue`] using default capacity hints.
  #[must_use]
  pub fn new() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY, DEFAULT_CAPACITY)
  }

  /// Creates a new, empty [`SendQueue`] with pre-reserved capacity for each
  /// priority lane. The capacities are **hints** — the queue is unbounded in
  /// Phase A and will grow as needed.
  #[must_use]
  pub fn with_capacity(system: usize, user: usize) -> Self {
    Self {
      system:      VecDeque::with_capacity(system),
      user:        VecDeque::with_capacity(user),
      user_paused: false,
    }
  }

  /// Enqueues `envelope` into the lane that matches its priority.
  pub fn offer(&mut self, envelope: OutboundEnvelope) -> OfferOutcome {
    match envelope.priority() {
      | OutboundPriority::System => self.system.push_back(envelope),
      | OutboundPriority::User => self.user.push_back(envelope),
    }
    OfferOutcome::Accepted
  }

  /// Returns the next envelope to send, honouring priority and backpressure.
  ///
  /// System-priority envelopes are drained first. User-priority envelopes are
  /// skipped while [`BackpressureSignal::Apply`] is in effect.
  pub fn next_outbound(&mut self) -> Option<OutboundEnvelope> {
    if let Some(env) = self.system.pop_front() {
      return Some(env);
    }
    if self.user_paused {
      return None;
    }
    self.user.pop_front()
  }

  /// Applies a backpressure signal from the transport layer.
  pub const fn apply_backpressure(&mut self, signal: BackpressureSignal) {
    match signal {
      | BackpressureSignal::Apply => self.user_paused = true,
      | BackpressureSignal::Release => self.user_paused = false,
    }
  }

  /// Returns `true` when the user lane is currently paused by backpressure.
  #[must_use]
  pub const fn is_user_paused(&self) -> bool {
    self.user_paused
  }

  /// Returns the combined length of the system and user lanes.
  #[must_use]
  pub fn len(&self) -> usize {
    self.system.len() + self.user.len()
  }

  /// Returns `true` when both lanes are empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.system.is_empty() && self.user.is_empty()
  }

  /// Drains the queue, returning all pending envelopes in priority order
  /// (system first, then user).
  pub fn drain_all(&mut self) -> Vec<OutboundEnvelope> {
    let mut out: Vec<OutboundEnvelope> = mem::take(&mut self.system).into();
    out.extend(mem::take(&mut self.user));
    out
  }
}

impl Default for SendQueue {
  fn default() -> Self {
    Self::new()
  }
}
