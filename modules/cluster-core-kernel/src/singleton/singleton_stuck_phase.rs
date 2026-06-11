//! Stuck phase vocabulary for singleton hand-over observation.

#[cfg(test)]
#[path = "singleton_stuck_phase_test.rs"]
mod tests;

/// Phase in which a singleton hand-over is stuck.
///
/// This enum represents the two possible stall phases observed when a
/// singleton hand-over is not progressing.  It is used as a payload field
/// in the `ClusterEvent::SingletonHandOverStuck` observation event.
///
/// # Observation contract
///
/// Receiving this value is a passive observation.  It MUST NOT be used as a
/// trigger for Membership State Transitions or Downing Decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingletonStuckPhase {
  /// Waiting to become the oldest member's singleton host.
  BecomingOldest,
  /// Hand-over to the next oldest member is in progress.
  HandingOver,
}
