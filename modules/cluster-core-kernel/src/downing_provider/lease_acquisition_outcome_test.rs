use crate::downing_provider::{DowningDecisionTrace, LeaseAcquisitionOutcome, SplitBrainResolverStrategy};

#[test]
fn lease_outcome_trace_mapping_keeps_acquired_reason() {
  let trace = DowningDecisionTrace::from_lease_outcome(
    SplitBrainResolverStrategy::LeaseMajority,
    LeaseAcquisitionOutcome::Acquired,
  );

  assert_eq!(trace.lease_outcome(), Some(LeaseAcquisitionOutcome::Acquired));
  assert_eq!(trace.reason(), "lease acquired for majority partition");
}

#[test]
fn lease_outcome_trace_mapping_distinguishes_failures() {
  let denied = DowningDecisionTrace::from_lease_outcome(
    SplitBrainResolverStrategy::LeaseMajority,
    LeaseAcquisitionOutcome::Denied,
  );
  let unavailable = DowningDecisionTrace::from_lease_outcome(
    SplitBrainResolverStrategy::LeaseMajority,
    LeaseAcquisitionOutcome::Unavailable,
  );
  let unknown = DowningDecisionTrace::from_lease_outcome(
    SplitBrainResolverStrategy::LeaseMajority,
    LeaseAcquisitionOutcome::Unknown,
  );
  let backend_missing = DowningDecisionTrace::from_lease_outcome(
    SplitBrainResolverStrategy::LeaseMajority,
    LeaseAcquisitionOutcome::BackendMissing,
  );

  assert_eq!(denied.reason(), "lease acquisition denied");
  assert_eq!(denied.lease_outcome(), Some(LeaseAcquisitionOutcome::Denied));
  assert_eq!(unavailable.reason(), "lease backend unavailable");
  assert_eq!(unavailable.lease_outcome(), Some(LeaseAcquisitionOutcome::Unavailable));
  assert_eq!(unknown.reason(), "lease acquisition outcome unknown");
  assert_eq!(unknown.lease_outcome(), Some(LeaseAcquisitionOutcome::Unknown));
  assert_eq!(backend_missing.reason(), "lease backend missing");
  assert_eq!(backend_missing.lease_outcome(), Some(LeaseAcquisitionOutcome::BackendMissing));
}
