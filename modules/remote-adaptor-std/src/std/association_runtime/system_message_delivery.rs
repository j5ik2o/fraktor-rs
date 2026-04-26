//! Per-association ack-based redelivery bookkeeping.

use std::collections::VecDeque;

use fraktor_remote_core_rs::core::wire::{AckPdu, EnvelopePdu};

/// State machine for ack-based system message delivery.
///
/// Phase B's minimum-viable implementation tracks the outgoing sequence
/// number, the cumulative ack received from the peer, and a pending window
/// of envelopes that have been sent but not yet acknowledged. The full
/// retransmission timer (with `tokio::time::sleep`) and nack handling are
/// declared as TODOs and will be filled in once the integration tests in
/// Section 24 require them.
#[derive(Debug)]
pub struct SystemMessageDeliveryState {
  /// Sequence number assigned to the next outbound system message.
  next_sequence:  u64,
  /// Highest sequence number cumulatively acknowledged by the peer.
  cumulative_ack: u64,
  /// Maximum number of unacknowledged messages allowed in flight.
  send_window:    u32,
  /// Pending unacked envelopes (in order).
  pending:        VecDeque<(u64, EnvelopePdu)>,
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
  pub fn record_send(&mut self, envelope: EnvelopePdu) -> Option<u64> {
    if self.is_window_full() {
      return None;
    }
    let seq = self.next_sequence;
    self.next_sequence = self.next_sequence.saturating_add(1);
    self.pending.push_back((seq, envelope));
    Some(seq)
  }

  /// Applies an inbound [`AckPdu`], removing acknowledged envelopes from the
  /// pending queue.
  pub fn apply_ack(&mut self, ack: &AckPdu) {
    let cumulative = ack.cumulative_ack();
    if cumulative > self.cumulative_ack {
      self.cumulative_ack = cumulative;
    }
    while let Some((seq, _)) = self.pending.front() {
      if *seq <= cumulative {
        self.pending.pop_front();
      } else {
        break;
      }
    }
  }
}
