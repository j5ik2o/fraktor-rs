//! Dual-priority queue owned by an [`crate::association::Association`].

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::mem;

use crate::{
  association::offer_outcome::OfferOutcome,
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::BackpressureSignal,
};

/// Default bounded capacity for each priority lane.
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
  system:       VecDeque<OutboundEnvelope>,
  user:         VecDeque<OutboundEnvelope>,
  system_limit: usize,
  user_limit:   usize,
  user_paused:  bool,
}

impl SendQueue {
  /// Creates a new, empty [`SendQueue`] using default bounded lane limits.
  #[must_use]
  pub fn new() -> Self {
    Self::with_limits(DEFAULT_CAPACITY, DEFAULT_CAPACITY)
  }

  /// Creates a new, empty [`SendQueue`] with bounded limits for each priority lane.
  ///
  /// This does not pre-allocate lane storage; limits and initial allocation are
  /// intentionally separate so per-association construction stays cheap.
  ///
  /// # Panics
  ///
  /// Panics when either limit is zero.
  #[must_use]
  pub fn with_limits(system_limit: usize, user_limit: usize) -> Self {
    assert!(system_limit > 0, "system queue capacity must be greater than zero");
    assert!(user_limit > 0, "user queue capacity must be greater than zero");
    Self { system: VecDeque::new(), user: VecDeque::new(), system_limit, user_limit, user_paused: false }
  }

  /// Creates a new, empty [`SendQueue`] with bounded capacity pre-allocated for each priority lane.
  ///
  /// # Panics
  ///
  /// Panics when either capacity is zero.
  #[must_use]
  pub fn with_capacity(system: usize, user: usize) -> Self {
    assert!(system > 0, "system queue capacity must be greater than zero");
    assert!(user > 0, "user queue capacity must be greater than zero");
    Self {
      system:       VecDeque::with_capacity(system),
      user:         VecDeque::with_capacity(user),
      system_limit: system,
      user_limit:   user,
      user_paused:  false,
    }
  }

  /// Enqueues `envelope` into the lane that matches its priority.
  pub fn offer(&mut self, envelope: OutboundEnvelope) -> OfferOutcome {
    match envelope.priority() {
      | OutboundPriority::System if self.system.len() < self.system_limit => self.system.push_back(envelope),
      | OutboundPriority::User if self.user.len() < self.user_limit => self.user.push_back(envelope),
      | _ => return OfferOutcome::QueueFull { envelope: Box::new(envelope) },
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
      | BackpressureSignal::Notify => {},
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
