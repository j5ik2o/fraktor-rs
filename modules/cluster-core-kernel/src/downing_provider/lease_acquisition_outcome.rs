//! Lease acquisition result vocabulary.

#[cfg(test)]
#[path = "lease_acquisition_outcome_test.rs"]
mod tests;

/// Result returned by a lease majority backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseAcquisitionOutcome {
  /// The lease was acquired for the majority partition.
  Acquired,
  /// The lease backend rejected the acquisition.
  Denied,
  /// The lease backend is unavailable.
  Unavailable,
  /// The lease backend could not determine the outcome.
  Unknown,
  /// No lease backend is configured.
  BackendMissing,
}

impl LeaseAcquisitionOutcome {
  /// Returns the trace reason associated with this outcome.
  #[must_use]
  pub const fn trace_reason(self) -> &'static str {
    match self {
      | Self::Acquired => "lease acquired for majority partition",
      | Self::Denied => "lease acquisition denied",
      | Self::Unavailable => "lease backend unavailable",
      | Self::Unknown => "lease acquisition outcome unknown",
      | Self::BackendMissing => "lease backend missing",
    }
  }
}
