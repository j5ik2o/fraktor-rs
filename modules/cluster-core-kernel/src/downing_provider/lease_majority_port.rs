//! Lease majority acquisition port.

#[cfg(test)]
#[path = "lease_majority_port_test.rs"]
mod tests;

use super::{DowningDecisionContext, LeaseAcquisitionOutcome};

/// Port used by core SBR evaluation to observe a lease acquisition result.
pub trait LeaseMajorityPort {
  /// Acquires or observes majority lease ownership for the current decision context.
  fn acquire_majority(&mut self, context: &DowningDecisionContext) -> LeaseAcquisitionOutcome;
}
