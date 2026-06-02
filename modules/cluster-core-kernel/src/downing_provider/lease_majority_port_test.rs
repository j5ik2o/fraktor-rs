use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  downing_provider::{DowningDecisionContext, LeaseAcquisitionOutcome, LeaseMajorityPort},
  membership::{DataCenter, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus},
};

struct RecordingLeasePort {
  outcome: LeaseAcquisitionOutcome,
  calls:   usize,
}

impl LeaseMajorityPort for RecordingLeasePort {
  fn acquire_majority(&mut self, context: &DowningDecisionContext) -> LeaseAcquisitionOutcome {
    assert!(context.membership_snapshot().is_some());
    self.calls += 1;
    self.outcome
  }
}

fn unique(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn context() -> DowningDecisionContext {
  let record = NodeRecord::new_with_identity(
    unique("node-a", 1),
    DataCenter::default(),
    String::from("node-a"),
    NodeStatus::Up,
    MembershipVersion::new(1),
    String::from("1.0.0"),
    vec![String::from("backend")],
  );
  DowningDecisionContext::from_membership_snapshot(
    MembershipSnapshot::new(MembershipVersion::new(1), vec![record]),
    TimerInstant::zero(Duration::from_millis(1)),
  )
}

#[test]
fn lease_majority_port_contract_uses_context_and_returns_outcome() {
  let mut port = RecordingLeasePort { outcome: LeaseAcquisitionOutcome::Unavailable, calls: 0 };

  let outcome = port.acquire_majority(&context());

  assert_eq!(outcome, LeaseAcquisitionOutcome::Unavailable);
  assert_eq!(port.calls, 1);
}
