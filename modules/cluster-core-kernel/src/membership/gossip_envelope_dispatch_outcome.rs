//! Gossip envelope dispatch outcome.

/// Result of checking whether an envelope can be dispatched now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GossipEnvelopeDispatchOutcome {
  /// The envelope is still within its dispatch deadline.
  Ready,
  /// The envelope deadline has passed and must not be treated as sent.
  DeadlineExpired {
    /// Deadline tick carried by the envelope.
    deadline_tick: u64,
    /// Current tick observed by the caller.
    now_tick:      u64,
  },
}
