//! Outcome of a grain readiness derivation.

use alloc::vec::Vec;

use super::GrainUnreadyReason;

/// Result of deriving grain readiness from a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrainReadiness {
  /// The grain runtime can accept traffic.
  Ready,
  /// The grain runtime cannot accept traffic yet.
  NotReady {
    /// All conditions that are not satisfied.
    reasons: Vec<GrainUnreadyReason>,
  },
}
