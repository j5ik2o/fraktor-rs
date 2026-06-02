//! std lease backend boundary used by the SBR provider binding.

use fraktor_cluster_core_kernel_rs::downing_provider::{DowningDecisionContext, LeaseAcquisitionOutcome};

/// std lease backend boundary used by the SBR provider binding.
pub trait StdLeaseMajorityBackend: Send + Sync {
  /// Attempts to acquire a lease for the evaluated majority partition.
  fn acquire(&mut self, context: &DowningDecisionContext) -> LeaseAcquisitionOutcome;

  /// Releases provider-owned backend state before the provider leaves its lifecycle.
  fn close(&mut self) {}
}
