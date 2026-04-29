//! Per-association ack-based redelivery bookkeeping.

use std::collections::VecDeque;

use fraktor_remote_core_rs::core::wire::{AckPdu, EnvelopePdu};

/// State machine for ack-based system message delivery.
///
/// The state tracks outgoing sequence numbers, the cumulative ack received
/// from the peer, pending envelopes that have not been acknowledged yet, and
/// the last time each pending envelope was emitted. The runtime driver owns
/// the actual timer and transport send side effects.
#[derive(Debug)]
pub struct SystemMessageDeliveryState {
  /// Sequence number assigned to the next outbound system message.
  next_sequence:  u64,
  /// Highest sequence number cumulatively acknowledged by the peer.
  cumulative_ack: u64,
  /// Maximum number of unacknowledged messages allowed in flight.
  send_window:    u32,
  /// Pending unacked envelopes (in order).
  pending:        VecDeque<PendingSystemMessage>,
}

#[derive(Debug)]
struct PendingSystemMessage {
  sequence_number: u64,
  envelope:        EnvelopePdu,
  last_sent_at_ms: u64,
}

impl PendingSystemMessage {
  fn new(sequence_number: u64, envelope: EnvelopePdu, last_sent_at_ms: u64) -> Self {
    Self { sequence_number, envelope, last_sent_at_ms }
  }

  fn is_due(&self, now_ms: u64, resend_interval_ms: u64) -> bool {
    now_ms.saturating_sub(self.last_sent_at_ms) >= resend_interval_ms
  }

  fn nacked_by(&self, ack: &AckPdu) -> bool {
    let Some(offset_from_cumulative) = self.sequence_number.checked_sub(ack.cumulative_ack()) else {
      return false;
    };
    if !(1..=u64::BITS as u64).contains(&offset_from_cumulative) {
      return false;
    }
    let bit_index = offset_from_cumulative - 1;
    ack.nack_bitmap() & (1_u64 << bit_index) != 0
  }
}

impl SystemMessageDeliveryState {
  /// Creates a fresh delivery state with the given send window.
  #[must_use]
  pub const fn new(send_window: u32) -> Self {
    Self { next_sequence: 1, cumulative_ack: 0, send_window, pending: VecDeque::new() }
  }

  /// Returns the configured send window size.
  #[must_use]
  pub const fn send_window(&self) -> u32 {
    self.send_window
  }

  /// Returns the next sequence number to be assigned.
  #[must_use]
  pub const fn next_sequence(&self) -> u64 {
    self.next_sequence
  }

  /// Returns the highest cumulative ack received so far.
  #[must_use]
  pub const fn cumulative_ack(&self) -> u64 {
    self.cumulative_ack
  }

  /// Returns the number of envelopes currently pending acknowledgement.
  #[must_use]
  pub fn pending_len(&self) -> usize {
    self.pending.len()
  }

  /// Returns `true` when the in-flight window is at the configured limit.
  #[must_use]
  pub fn is_window_full(&self) -> bool {
    self.pending.len() as u32 >= self.send_window
  }

  /// Records that `envelope` has been emitted, returning its assigned
  /// sequence number.
  ///
  /// Returns `None` when the in-flight window is full and the envelope must
  /// be deferred to a later attempt.
  pub fn record_send(&mut self, envelope: EnvelopePdu, now_ms: u64) -> Option<u64> {
    if self.is_window_full() {
      return None;
    }
    let seq = self.next_sequence;
    self.next_sequence = self.next_sequence.saturating_add(1);
    self.pending.push_back(PendingSystemMessage::new(seq, envelope, now_ms));
    Some(seq)
  }

  /// Returns pending envelopes whose last send time has reached
  /// `resend_interval_ms`.
  #[must_use]
  pub fn due_retransmissions(&self, now_ms: u64, resend_interval_ms: u64) -> Vec<(u64, EnvelopePdu)> {
    self
      .pending
      .iter()
      .filter(|entry| entry.is_due(now_ms, resend_interval_ms))
      .map(|entry| (entry.sequence_number, entry.envelope.clone()))
      .collect()
  }

  /// Marks a pending envelope as retransmitted at `now_ms`.
  ///
  /// Returns `false` when `sequence_number` is no longer pending.
  pub fn mark_retransmitted(&mut self, sequence_number: u64, now_ms: u64) -> bool {
    let Some(entry) = self.pending.iter_mut().find(|entry| entry.sequence_number == sequence_number) else {
      return false;
    };
    entry.last_sent_at_ms = now_ms;
    true
  }

  /// Returns pending envelopes selected by the nack bitmap in `ack`.
  #[must_use]
  pub fn nacked_pending(&self, ack: &AckPdu) -> Vec<(u64, EnvelopePdu)> {
    self
      .pending
      .iter()
      .filter(|entry| entry.nacked_by(ack))
      .map(|entry| (entry.sequence_number, entry.envelope.clone()))
      .collect()
  }

  /// Applies an inbound [`AckPdu`], removing acknowledged envelopes from the
  /// pending queue.
  pub fn apply_ack(&mut self, ack: &AckPdu) {
    let cumulative = ack.cumulative_ack();
    if cumulative > self.cumulative_ack {
      self.cumulative_ack = cumulative;
    }
    while let Some(entry) = self.pending.front() {
      if entry.sequence_number <= cumulative {
        self.pending.pop_front();
      } else {
        break;
      }
    }
  }
}
