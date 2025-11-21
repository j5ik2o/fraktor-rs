//! Outbound message pipeline with buffering and quarantine controls.

use alloc::{collections::VecDeque, string::{String, ToString}, vec::Vec};

use crate::core::{outbound_action::OutboundAction, outbound_envelope::OutboundEnvelope, outbound_event::OutboundEvent, outbound_state::OutboundState};

#[cfg(test)]
mod tests;

/// Manages send ordering, buffering, and quarantine for a single authority.
pub struct OutboundPipeline {
  authority: String,
  capacity: usize,
  state: OutboundState,
  queue: VecDeque<OutboundEnvelope>,
  events: Vec<OutboundEvent>,
}

impl OutboundPipeline {
  /// Creates a new pipeline.
  pub fn new(authority: String, capacity: usize) -> Self {
    Self {
      authority,
      capacity,
      state: OutboundState::Disconnected,
      queue: VecDeque::new(),
      events: Vec::new(),
    }
  }

  /// Returns the current state.
  pub const fn state(&self) -> &OutboundState {
    &self.state
  }

  /// Attempts to send an envelope.
  pub fn send(&mut self, envelope: OutboundEnvelope) -> OutboundAction {
    match self.state {
      | OutboundState::Connected => {
        self.events.push(OutboundEvent::Dispatched { pid: envelope.pid.clone() });
        OutboundAction::Immediate { envelope }
      },
      | OutboundState::Disconnected => {
        if self.capacity == 0 {
          self.events.push(OutboundEvent::DroppedOldest { dropped: envelope.clone(), reason: "queue overflow".to_string() });
          return OutboundAction::DroppedOldest { dropped: envelope, queue_len: 0 };
        }

        if self.queue.len() >= self.capacity {
          let dropped = self.queue.pop_front().expect("queue has at least one element");
          self.events.push(OutboundEvent::DroppedOldest { dropped: dropped.clone(), reason: "queue overflow".to_string() });
          self.queue.push_back(envelope);
          return OutboundAction::DroppedOldest { dropped, queue_len: self.queue.len() };
        }

        self.events.push(OutboundEvent::Enqueued { pid: envelope.pid.clone(), queue_len: self.queue.len() + 1 });
        self.queue.push_back(envelope);
        OutboundAction::Enqueued { queue_len: self.queue.len() }
      },
      | OutboundState::Quarantine { ref reason, .. } => {
        self.events.push(OutboundEvent::BlockedByQuarantine { pid: envelope.pid, reason: reason.clone() });
        OutboundAction::RejectedQuarantine { reason: reason.clone() }
      },
    }
  }

  /// Marks the authority connected and flushes the queue.
  pub fn set_connected(&mut self) -> Vec<OutboundEnvelope> {
    self.state = OutboundState::Connected;
    let drained: Vec<_> = self.queue.drain(..).collect();
    if !drained.is_empty() {
      self.events.push(OutboundEvent::Flushed { delivered: drained.len() });
    }
    drained
  }

  /// Marks the authority as disconnected.
  pub fn set_disconnected(&mut self) {
    self.state = OutboundState::Disconnected;
  }

  /// Starts quarantine.
  pub fn set_quarantine(&mut self, reason: String, deadline: Option<u64>) {
    self.state = OutboundState::Quarantine { reason: reason.clone(), deadline };
    self.events.push(OutboundEvent::Quarantined { authority: self.authority.clone(), reason, deadline });
  }

  /// Polls quarantine expiration.
  pub fn poll_quarantine_expiration(&mut self, now: u64) -> bool {
    if let OutboundState::Quarantine { deadline: Some(deadline), .. } = self.state {
      if now >= deadline {
        self.state = OutboundState::Disconnected;
        self.events.push(OutboundEvent::QuarantineLifted { authority: self.authority.clone() });
        return true;
      }
    }
    false
  }

  /// Records serialization failure.
  pub fn record_serialization_failure(&mut self, pid: String, reason: String) {
    self.events.push(OutboundEvent::SerializationFailed { pid, reason });
  }

  /// Drains buffered events.
  pub fn drain_events(&mut self) -> Vec<OutboundEvent> {
    core::mem::take(&mut self.events)
  }
}
